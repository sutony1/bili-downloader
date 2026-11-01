use crate::{command, download_process::ManagedChild, Msg};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use tao::event_loop::EventLoopProxy;
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    },
};
use wry::WebView;

#[derive(Default)]
pub struct Transcription {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    cancel: Option<Arc<AtomicBool>>,
}

fn update(ui: &WebView, mut data: Value) {
    data["platform"] = json!("transcribe");
    let _ = ui.evaluate_script(&format!("window.updateTranscription({data});"));
}

fn pick_file() -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    let filter: Vec<u16> = "录音和视频文件\0*.m4a;*.mp3;*.wav;*.aac;*.flac;*.ogg;*.opus;*.wma;*.amr;*.mp4;*.mkv;*.webm;*.mov\0所有文件\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = "打开录音或视频文件\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        let end = buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len());
        Ok(Some(PathBuf::from(String::from_utf16_lossy(
            &buffer[..end],
        ))))
    } else {
        let error = unsafe { CommDlgExtendedError() }.0;
        if error == 0 {
            Ok(None)
        } else {
            Err(format!("文件选择器错误：{error}"))
        }
    }
}

fn python() -> String {
    if let Ok(p) = std::env::var("BILI_WHISPER_PYTHON") {
        return p;
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.parent().unwrap().join("transcribe-runtime/python.exe");
        if p.is_file() {
            return p.to_string_lossy().into();
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let prepared =
            PathBuf::from(&local).join("BiliDownloader/Transcription/venv/Scripts/python.exe");
        if prepared.is_file() {
            return prepared.to_string_lossy().into();
        }
        for version in ["Python312", "Python311", "Python313"] {
            let p = PathBuf::from(&local)
                .join("Programs/Python")
                .join(version)
                .join("python.exe");
            if p.is_file() {
                return p.to_string_lossy().into();
            }
        }
    }
    "python".into()
}

impl Transcription {
    pub fn set_test_file(&mut self, path: PathBuf, ui: &WebView) {
        self.input = Some(path.clone());
        update(ui, json!({"input":path}));
    }
    pub fn ui(
        &mut self,
        v: &Value,
        ui: &WebView,
        proxy: &EventLoopProxy<Msg>,
        busy: &Arc<AtomicBool>,
    ) {
        let action = v["action"].as_str().unwrap_or("");
        if action == "transcribe-stop" {
            if let Some(cancel) = &self.cancel {
                cancel.store(true, Ordering::Relaxed);
                update(ui, json!({"status":"正在停止转写…","stopping":true}));
            }
            return;
        }
        if action == "transcribe-open-txt" || action == "transcribe-folder" {
            if let Some(path) = &self.output {
                if path.is_file() {
                    if action == "transcribe-open-txt" {
                        let _ = command("notepad.exe").arg(path).spawn();
                    } else {
                        let _ = command("explorer.exe")
                            .arg(format!("/select,{}", path.display()))
                            .spawn();
                    }
                }
            }
            return;
        }
        if action == "platform-selected" {
            return;
        }
        if busy.load(Ordering::Relaxed) {
            return;
        }
        if action == "transcribe-prepare" {
            if let Ok(exe) = std::env::current_exe() {
                let script = exe.with_file_name("prepare-transcription.ps1");
                if script.is_file() {
                    let _ = std::process::Command::new("powershell.exe")
                        .args(["-NoExit", "-ExecutionPolicy", "Bypass", "-File"])
                        .arg(script)
                        .spawn();
                    update(
                        ui,
                        json!({"status":"请在打开的准备窗口完成安装，之后点击检查环境"}),
                    );
                } else {
                    update(
                        ui,
                        json!({"status":"未找到准备脚本，请使用完整解压的 v0.4.0 程序包"}),
                    );
                }
            }
        } else if action == "transcribe-pick" {
            match pick_file() {
                Ok(Some(path)) => {
                    self.input = Some(path.clone());
                    self.output = None;
                    update(
                        ui,
                        json!({"input":path,"output":"","progress":0,"status":"录音已选择，可以开始转写","preview":"","log":"TXT 将保存在录音旁；同名文件自动编号，不覆盖已有文件。"}),
                    );
                }
                Ok(None) => {}
                Err(error) => update(ui, json!({"status":"无法打开文件","log":error})),
            }
        } else if action == "transcribe-start" || action == "transcribe-check" {
            let check = action == "transcribe-check";
            if !check && self.input.is_none() {
                update(ui, json!({"status":"请先打开录音文件"}));
                return;
            }
            if busy
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_err()
            {
                return;
            }
            let language = match v["language"].as_str() {
                Some("zh") => "zh",
                Some("en") => "en",
                _ => "auto",
            };
            let input = self.input.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            self.cancel = Some(cancel.clone());
            let proxy = proxy.clone();
            let busy = busy.clone();
            update(
                ui,
                json!({"busy":true,"stopping":false,"status":if check {"正在检查环境…"} else {"正在启动 GPU 转写…"},"progress":0,"preview":"","log":"本地处理，不上传录音。首次加载 large-v3 需要稍等。"}),
            );
            std::thread::spawn(move || {
                let result = run_worker(check, input, language, &cancel, &proxy);
                busy.store(false, Ordering::Relaxed);
                let value = match result {
                    Ok(()) => json!({"busy":false,"stopping":false}),
                    Err(_error) if cancel.load(Ordering::Relaxed) => {
                        json!({"busy":false,"stopping":false,"status":"转写已停止","log":"未完成的识别未保存为 TXT。"})
                    }
                    Err(error) => {
                        json!({"busy":false,"stopping":false,"status":"转写失败","log":error})
                    }
                };
                let _ = proxy.send_event(Msg::Transcription(value));
            });
        }
    }
    pub fn event(&mut self, value: Value, ui: &WebView) {
        if let Some(path) = value["output"].as_str() {
            self.output = Some(PathBuf::from(path));
        }
        if value["busy"] == false {
            self.cancel = None;
        }
        update(ui, value);
    }
    pub fn stop(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn run_worker(
    check: bool,
    input: Option<PathBuf>,
    language: &str,
    cancel: &AtomicBool,
    proxy: &EventLoopProxy<Msg>,
) -> Result<(), String> {
    let root = PathBuf::from(std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?)
        .join("BiliDownloader/Transcription");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let script = root.join("worker-v0.4.0.py");
    std::fs::write(&script, include_str!("../transcribe_audio.py")).map_err(|e| e.to_string())?;
    let log = root.join("last-error.log");
    let stderr = std::fs::File::create(&log).map_err(|e| e.to_string())?;
    let mut cmd = command(&python());
    cmd.arg("-u")
        .arg(&script)
        .env("PYTHONUTF8", "1")
        .env("BILI_WHISPER_FFMPEG", crate::tool("ffmpeg"))
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .stdin(Stdio::null());
    if check {
        cmd.arg("--check");
    } else {
        cmd.arg("--input")
            .arg(input.unwrap())
            .arg("--language")
            .arg(language);
    }
    let mut child = ManagedChild::spawn(&mut cmd)
        .map_err(|e| format!("无法启动转写环境：{e}。请运行“准备语音转写环境.cmd”。"))?;
    let stdout = child.stdout.take().ok_or("无法读取转写进度")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut error = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            child.terminate();
            return Err("转写已停止".into());
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(line)) => {
                if let Ok(mut value) = serde_json::from_str::<Value>(&line) {
                    if value["error"] == true {
                        error = value["log"].as_str().map(str::to_owned);
                    }
                    // Worker completion is final only after its process exits.
                    value["busy"] = json!(true);
                    let _ = proxy.send_event(Msg::Transcription(value));
                }
            }
            Ok(Err(e)) => {
                error = Some(e.to_string());
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || error.is_some() {
        let detail = std::fs::read_to_string(&log).unwrap_or_default();
        return Err(error.unwrap_or_else(|| {
            format!(
                "GPU 转写进程退出（{status}）：{}",
                detail.chars().take(1800).collect::<String>()
            )
        }));
    }
    Ok(())
}
