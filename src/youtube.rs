use super::*;
use std::io::{BufRead, Read};
use tao::{event_loop::EventLoop, window::Window};

pub fn video_id(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw.trim()).ok()?;
    if u.scheme() != "https" || u.port().is_some() || !u.username().is_empty() {
        return None;
    }
    let host = u.host_str()?;
    let id = if host == "youtu.be" {
        u.path().trim_matches('/').to_owned()
    } else if [
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "music.youtube.com",
    ]
    .contains(&host)
    {
        if u.path() == "/watch" {
            u.query_pairs().find(|(k, _)| k == "v")?.1.into_owned()
        } else {
            let p: Vec<_> = u.path_segments()?.collect();
            if p.len() != 2 || !["shorts", "live", "embed"].contains(&p[0]) {
                return None;
            }
            p[1].to_owned()
        }
    } else {
        return None;
    };
    (id.len() == 11
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'))
    .then_some(id)
}
fn allowed_navigation(raw: &str) -> bool {
    raw == "about:blank"
        || host_allowed(raw, "youtube.com")
        || host_allowed(raw, "youtu.be")
        || host_allowed(raw, "accounts.google.com")
        || host_allowed(raw, "consent.google.com")
}
fn show(ui: &WebView, mut v: Value) {
    v["platform"] = json!("youtube");
    let _ = ui.evaluate_script(&format!("window.update({v});"));
}
fn executable() -> String {
    let next = std::env::current_exe()
        .unwrap()
        .with_file_name("yt-dlp.exe");
    if next.exists() {
        return next.to_string_lossy().into();
    }
    let dev = std::env::current_dir()
        .unwrap_or_default()
        .join("tools")
        .join("yt-dlp.exe");
    if dev.exists() {
        return dev.to_string_lossy().into();
    }
    "yt-dlp".into()
}
fn base_command(url: &str, job: &Job) -> Result<Command, String> {
    let mut c = command(&executable());
    c.args([
        "--ignore-config",
        "--encoding",
        "utf-8",
        "--no-playlist",
        "--no-colors",
        "--socket-timeout",
        "20",
        "--retries",
        "1",
        "--fragment-retries",
        "2",
        "--no-cache-dir",
        "--no-warnings",
    ]);
    let deno = std::env::current_exe().unwrap().with_file_name("deno.exe");
    let node = std::env::current_exe().unwrap().with_file_name("node.exe");
    if deno.exists() {
        c.args(["--js-runtimes", &format!("deno:{}", deno.display())]);
    } else if node.exists() {
        c.args(["--js-runtimes", &format!("node:{}", node.display())]);
    } else if command("deno")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        c.args(["--js-runtimes", "deno"]);
    } else if command("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        c.args(["--js-runtimes", "node"]);
    } else {
        return Err("需要 JavaScript 运行组件。请双击程序旁的“准备YouTube组件.cmd”，或按 README 准备 Deno / Node.js 22+。".into());
    }
    if job.cookie_file.exists() {
        c.arg("--cookies").arg(&job.cookie_file);
    }
    // urllib/yt-dlp also honors the user's existing Windows proxy configuration.
    // Do not modify system proxy settings or invent a proxy address.
    c.arg(url);
    Ok(c)
}
struct Job {
    dir: PathBuf,
    cookie_file: PathBuf,
}
impl Job {
    fn new(cookies: &str) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = PathBuf::from(std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?)
            .join("BiliDownloader")
            .join("YouTubeJobs")
            .join(format!("{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let cookie_file = dir.join("cookies.txt");
        if !cookies.is_empty() {
            std::fs::write(&cookie_file, cookies).map_err(|e| e.to_string())?;
        }
        Ok(Self { dir, cookie_file })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.cookie_file);
        let _ = std::fs::remove_dir(&self.dir);
    }
}
fn sanitized_error(raw: &str) -> String {
    let lines = raw
        .lines()
        .filter(|s| s.starts_with("ERROR:"))
        .collect::<Vec<_>>();
    let s = if lines.is_empty() {
        "YouTube 请求失败，请检查网络、组件版本或登录状态。".to_owned()
    } else {
        lines.join("\n")
    };
    s.split_whitespace()
        .map(|w| {
            if w.contains("https://") || w.contains("http://") {
                "[网址]"
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(1800)
        .collect()
}
fn run_tool(
    mut c: Command,
    timeout: Duration,
    mut line: impl FnMut(&str),
) -> Result<String, String> {
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|_| {
        "未找到 yt-dlp.exe。请双击程序旁的“准备YouTube组件.cmd”，完成后重试。".to_owned()
    })?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout).take(25 * 1024 * 1024);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            match reader.read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let s = String::from_utf8_lossy(&bytes)
                        .trim_end_matches(['\r', '\n'])
                        .to_owned();
                    if tx.send(s).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(128 * 1024).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let start = Instant::now();
    let mut text = String::new();
    let status = loop {
        while let Ok(s) = rx.try_recv() {
            line(&s);
            if text.len() < 25 * 1024 * 1024 {
                text.push_str(&s);
                text.push('\n');
            }
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            let _ = errors.join();
            return Err(
                "YouTube 请求超时，请检查网络，或在内置浏览器确认视频能够播放后重试。".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let _ = reader.join();
    while let Ok(s) = rx.try_recv() {
        line(&s);
        if text.len() < 25 * 1024 * 1024 {
            text.push_str(&s);
            text.push('\n');
        }
    }
    let error = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(sanitized_error(&error));
    }
    Ok(text)
}
fn format_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub fn normalize(data: &Value, expected: &str) -> Result<Value, String> {
    if data["id"].as_str() != Some(expected) {
        return Err("视频编号不一致，请重新解析。".into());
    }
    if data["is_live"] == true || data["live_status"] == "is_upcoming" {
        return Err("第一版暂不支持正在直播或尚未开始的视频。".into());
    }
    let duration = data["duration"]
        .as_f64()
        .filter(|d| d.is_finite() && *d > 0.)
        .ok_or("无法取得完整视频时长")?;
    let has_audio = data["formats"].as_array().is_some_and(|a| {
        a.iter()
            .any(|f| f["acodec"].as_str().is_some_and(|v| v != "none"))
    });
    let mut formats:Vec<Value>=data["formats"].as_array().ok_or("未返回画质列表")?.iter().filter_map(|f|{
        let id=f["format_id"].as_str()?;let codec=f["vcodec"].as_str()?;let w=f["width"].as_u64()?;let h=f["height"].as_u64()?;
        if !format_key(id)||codec=="none"||w==0||h==0||f["has_drm"]==true{return None;}
        if f["acodec"]=="none"&&!has_audio{return None;}
        let family=if codec.starts_with("avc"){"H.264"}else if codec.starts_with("av01"){"AV1"}else if codec.starts_with("vp"){"VP9"}else{codec};
        let ext=if f["ext"]=="webm"{"mkv"}else{"mp4"};let fps=f["fps"].as_f64().unwrap_or(0.);
        Some(json!({"id":id,"width":w,"height":h,"fps":fps,"codec":family,"ext":ext,"audio":f["acodec"]!="none","direct":f["protocol"]=="https","bitrate":f["tbr"].as_f64().unwrap_or(0.),"label":format!("{w}×{h} · {fps:.0} fps · {family} · {}",ext.to_uppercase())}))
    }).collect();
    formats.sort_by(|a, b| {
        let area = |f: &Value| f["width"].as_u64().unwrap_or(0) * f["height"].as_u64().unwrap_or(0);
        area(b)
            .cmp(&area(a))
            .then_with(|| {
                b["fps"]
                    .as_f64()
                    .unwrap_or(0.)
                    .total_cmp(&a["fps"].as_f64().unwrap_or(0.))
            })
            .then_with(|| (b["codec"] == "H.264").cmp(&(a["codec"] == "H.264")))
            .then_with(|| b["direct"].as_bool().cmp(&a["direct"].as_bool()))
            .then_with(|| {
                b["bitrate"]
                    .as_f64()
                    .unwrap_or(0.)
                    .total_cmp(&a["bitrate"].as_f64().unwrap_or(0.))
            })
    });
    let mut seen = std::collections::HashSet::new();
    formats.retain(|f| {
        seen.insert((
            f["width"].to_string(),
            f["height"].to_string(),
            f["fps"].to_string(),
            f["codec"].to_string(),
            f["ext"].to_string(),
        ))
    });
    formats.truncate(60);
    if formats.is_empty() {
        return Err(
            "未取得可下载的非加密视频画质。请更新 yt-dlp，或登录后重试；本版不处理受保护内容。"
                .into(),
        );
    }
    Ok(
        json!({"id":expected,"title":data["title"].as_str().unwrap_or("YouTube视频"),"duration":duration,"formats":formats,"has_audio":has_audio}),
    )
}
fn parse(raw: &str, cookies: &str) -> Result<Value, String> {
    let id = video_id(raw).ok_or("请输入有效的 YouTube 单条视频网址")?;
    let url = format!("https://www.youtube.com/watch?v={id}");
    let job = Job::new(cookies)?;
    let mut c = base_command(&url, &job)?;
    c.args(["--skip-download", "--dump-single-json", "--no-progress"]);
    let text = run_tool(c, Duration::from_secs(110), |_| {})?;
    let data: Value =
        serde_json::from_str(&text).map_err(|_| "视频信息解析失败，请更新 yt-dlp 后重试。")?;
    normalize(&data, &id)
}
fn download_video(
    data: &Value,
    format: &Value,
    cookies: &str,
    dir: &PathBuf,
    report: impl Fn(Value),
) -> Result<Value, String> {
    let id = data["id"].as_str().ok_or("缺少视频编号")?;
    let key = format["id"]
        .as_str()
        .filter(|s| format_key(s))
        .ok_or("画质编号无效")?;
    let ext = format["ext"]
        .as_str()
        .filter(|s| ["mp4", "mkv"].contains(s))
        .ok_or("容器无效")?;
    for tool_name in ["ffmpeg", "ffprobe"] {
        if !command(&tool(tool_name))
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
        {
            return Err(format!("需要 {tool_name}.exe，请按 README 放在程序旁边。"));
        }
    }
    let title = data["title"]
        .as_str()
        .unwrap_or("YouTube视频")
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*%".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(65)
        .collect::<String>();
    let base = format!("{} [YT{id}]", title.trim_end_matches([' ', '.']));
    let mut out = dir.join(format!("{base}.{ext}"));
    let mut n = 1;
    while out.exists() {
        out = dir.join(format!("{base} ({n}).{ext}"));
        n += 1;
    }
    let job = Job::new(cookies)?;
    let url = format!("https://www.youtube.com/watch?v={id}");
    let mut c = base_command(&url, &job)?;
    let selector = if format["audio"] == true {
        key.to_owned()
    } else {
        format!("{key}+bestaudio[ext=m4a]/{key}+bestaudio")
    };
    c.args(["-f",&selector,"--merge-output-format",ext,"--no-overwrites","--newline","--progress","--progress-delta","1","--progress-template","download:__YTPROGRESS__%(progress._percent_str)s | %(progress._speed_str)s | 剩余 %(progress._eta_str)s","--print","after_move:__YTOUT__%(filepath)j","--no-simulate","-o"]).arg(&out);
    let ffmpeg = tool("ffmpeg");
    if PathBuf::from(&ffmpeg).exists() {
        c.arg("--ffmpeg-location").arg(&ffmpeg);
    }
    let mut final_file = None;
    run_tool(c, Duration::from_secs(3600), |line| {
        if let Some(s) = line.strip_prefix("__YTPROGRESS__") {
            report(json!({"status":format!("正在下载 YouTube · {}",s.trim()),"busy":true}));
        } else if let Some(s) = line.strip_prefix("__YTOUT__") {
            final_file = serde_json::from_str::<String>(s).ok().map(PathBuf::from);
        }
    })?;
    let file = final_file.ok_or("下载进程未返回保存文件，请查看下载文件夹。")?;
    if file != out {
        return Err("下载结果的文件路径与预期不一致，未继续处理。".into());
    }
    report(json!({"status":"正在核验 YouTube 视频…","busy":true}));
    let probe = command(&tool("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,size:stream=codec_type,codec_name,width,height",
            "-of",
            "json",
        ])
        .arg(&file)
        .output()
        .map_err(|e| e.to_string())?;
    if !probe.status.success() {
        return Err("文件已保存，但媒体核验失败，请重新解析后重试。".into());
    }
    let media: Value = serde_json::from_slice(&probe.stdout).map_err(|e| e.to_string())?;
    let streams = media["streams"].as_array().ok_or("没有媒体流")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("没有视频流")?;
    if data["has_audio"] == true && !streams.iter().any(|s| s["codec_type"] == "audio") {
        return Err("下载结果缺少原视频音轨，请重新解析。".into());
    }
    let duration = media["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or("无法核验完整时长")?;
    if !duration.is_finite()
        || duration <= 0.
        || (duration - data["duration"].as_f64().unwrap_or(0.)).abs() > 3.
    {
        return Err("文件时长与源视频不一致，文件已保留。".into());
    }
    if video["width"] != format["width"] || video["height"] != format["height"] {
        return Err("保存文件的分辨率与所选画质不一致，请重新解析。".into());
    }
    let size = std::fs::metadata(&file).map_err(|e| e.to_string())?.len();
    Ok(
        json!({"status":"下载完成 ✓","ready":true,"info":format!("{}×{} · {duration:.1}秒 · {:.1}MB",video["width"],video["height"],size as f64/1048576.),"log":format!("保存位置：{}\n所选分辨率、音视频流和完整时长核验通过；未重新压缩。",file.display())}),
    )
}
pub struct YouTube {
    pub window: Window,
    pub browser: WebView,
    _context: WebContext,
    selected: Option<Value>,
    current_url: String,
    request: u64,
    next_login: Instant,
    login_open: bool,
    login_pending: Arc<AtomicBool>,
    use_login: bool,
    test_url: Option<String>,
    test_started: bool,
    pub finished_test: bool,
    test_download: bool,
    boot: Instant,
}
impl YouTube {
    pub fn new(events: &EventLoop<Msg>) -> Result<Self, Box<dyn std::error::Error>> {
        let window = WindowBuilder::new()
            .with_title("YouTube 登录 — 视频下载助手")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(1100., 800.))
            .build(events)?;
        let profile = if std::env::args().any(|a| a == "--isolated-test-profile") {
            std::env::current_dir()?
                .join("verification")
                .join("YouTubeTestProfile")
        } else {
            PathBuf::from(std::env::var("LOCALAPPDATA")?)
                .join("BiliDownloader")
                .join("YouTubeBrowserProfile")
        };
        std::fs::create_dir_all(&profile)?;
        let mut context = WebContext::new(Some(profile));
        let p = events.create_proxy();
        let browser = WebViewBuilder::new_with_web_context(&mut context)
            .with_url("about:blank")
            .with_navigation_handler(|u| allowed_navigation(&u))
            .with_new_window_req_handler(move |u, _| {
                if allowed_navigation(&u) {
                    let _ = p.send_event(Msg::YouTubeOpen(u));
                }
                NewWindowResponse::Deny
            })
            .build(&window)?;
        mute_browser(&browser)?;
        let test_url = std::env::args()
            .position(|a| a == "--youtube-smoke-test")
            .and_then(|i| std::env::args().nth(i + 1));
        Ok(Self {
            window,
            browser,
            _context: context,
            selected: None,
            current_url: String::new(),
            request: 0,
            next_login: Instant::now(),
            login_open: false,
            login_pending: Arc::new(AtomicBool::new(false)),
            use_login: false,
            test_url,
            test_started: false,
            finished_test: false,
            test_download: std::env::args().any(|a| a == "--download-test"),
            boot: Instant::now(),
        })
    }
    pub fn stop(&self) {
        stop_preview(&self.browser, None);
    }
    pub fn close_browser(&mut self) {
        self.login_open = false;
        self.stop();
        self.window.set_visible(false);
    }
    fn with_cookies(
        &self,
        enabled: bool,
        done: impl FnOnce(Result<String, String>) + Send + 'static,
    ) {
        if !enabled {
            done(Ok(String::new()));
            return;
        }
        browser_cookies::read(&self.browser, "https://www.youtube.com/", move |result| {
            done(result.map(Self::cookie_file));
        });
    }
    fn cookie_file(cookies: Vec<browser_cookies::BrowserCookie>) -> String {
        let mut output = String::from("# Netscape HTTP Cookie File\n");
        for c in cookies {
            let domain = c.domain.as_str();
            if domain.trim_start_matches('.') != "youtube.com" && !domain.ends_with(".youtube.com")
            {
                continue;
            }
            let path = c.path.as_str();
            let fields = [domain, path, &c.name, &c.value];
            if fields.iter().any(|s| s.contains(['\r', '\n', '\t'])) {
                continue;
            }
            let expiry = c.expires;
            let d = if c.http_only {
                format!("#HttpOnly_{domain}")
            } else {
                domain.to_owned()
            };
            output.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                d,
                if domain.starts_with('.') {
                    "TRUE"
                } else {
                    "FALSE"
                },
                path,
                if c.secure { "TRUE" } else { "FALSE" },
                expiry,
                c.name,
                c.value
            ));
        }
        output
    }
    pub fn ui(
        &mut self,
        v: &Value,
        ui: &WebView,
        proxy: &tao::event_loop::EventLoopProxy<Msg>,
        busy: &Arc<AtomicBool>,
        downloads: &PathBuf,
    ) {
        match v["action"].as_str().unwrap_or("") {
            "login" => {
                self.login_open = true;
                let url = if self.current_url.is_empty() {
                    "https://www.youtube.com/"
                } else {
                    &self.current_url
                };
                let _ = self.browser.load_url(url);
                self.window.set_visible(true);
                self.window.set_focus();
                self.next_login = Instant::now() + Duration::from_secs(3);
            }
            "folder" => {
                let _ = command("explorer.exe").arg(downloads).spawn();
            }
            "platform-selected" => self.stop(),
            "clear" if !busy.load(Ordering::Relaxed) => {
                self.selected = None;
                self.request += 1;
                self.stop();
            }
            "parse" if !busy.load(Ordering::Relaxed) => {
                self.selected = None;
                let Some(id) = video_id(v["url"].as_str().unwrap_or("")) else {
                    show(
                        ui,
                        json!({"status":"请输入 YouTube 单条视频链接","ready":false,"busy":false,"formats":[],"log":"支持 watch、youtu.be 分享链接和 Shorts；暂不支持播放列表或直播。"}),
                    );
                    return;
                };
                self.current_url = format!("https://www.youtube.com/watch?v={id}");
                self.request += 1;
                self.use_login = v["useLogin"] == true;
                let raw = self.current_url.clone();
                let request = self.request;
                let p = proxy.clone();
                let lock = busy.clone();
                busy.store(true, Ordering::Relaxed);
                show(
                    ui,
                    json!({"status":"正在解析 YouTube 视频…","ready":false,"busy":true,"formats":[],"info":"正在获取可用画质","log":"首次解析可能需要几十秒。使用电脑已有网络配置，不改变系统设置。"}),
                );
                self.with_cookies(self.use_login, move |cookies| {
                    std::thread::spawn(move || {
                        let result = cookies.and_then(|cookies| parse(&raw, &cookies));
                        lock.store(false, Ordering::Relaxed);
                        let _ = p.send_event(Msg::YouTubeParsed(request, result));
                    });
                });
            }
            "download" if !busy.load(Ordering::Relaxed) => {
                let Some(data) = self.selected.clone() else {
                    return;
                };
                let key = v["quality"].as_str().unwrap_or("");
                let Some(f) = data["formats"]
                    .as_array()
                    .and_then(|a| a.iter().find(|f| f["id"].as_str() == Some(key)))
                    .cloned()
                else {
                    return;
                };
                let dir = downloads.clone();
                let p = proxy.clone();
                let lock = busy.clone();
                busy.store(true, Ordering::Relaxed);
                self.stop();
                show(
                    ui,
                    json!({"status":"开始下载 YouTube 视频","busy":true,"ready":false}),
                );
                self.with_cookies(self.use_login, move |cookies| {
                    std::thread::spawn(move || {
                        let result = cookies.and_then(|cookies| {
                            download_video(&data, &f, &cookies, &dir, |mut x| {
                                x["platform"] = json!("youtube");
                                let _ = p.send_event(Msg::Progress(x));
                            })
                        });
                        lock.store(false, Ordering::Relaxed);
                        let mut x = result.unwrap_or_else(
                            |error| json!({"status":"YouTube 下载失败","log":error,"ready":true}),
                        );
                        x["platform"] = json!("youtube");
                        x["busy"] = json!(false);
                        let _ = p.send_event(Msg::DownloadFinished(x));
                    });
                });
            }
            _ => {}
        }
    }
    fn write_test(&self, value: Value) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::write(
                exe.with_file_name("youtube-test.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            );
        }
    }
    pub fn parsed(
        &mut self,
        request: u64,
        result: Result<Value, String>,
        ui: &WebView,
        proxy: &tao::event_loop::EventLoopProxy<Msg>,
        busy: &Arc<AtomicBool>,
        downloads: &PathBuf,
    ) {
        if request != self.request {
            return;
        }
        match result {
            Ok(data) => {
                let formats: Vec<_> = data["formats"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| json!({"id":f["id"],"label":f["label"]}))
                    .collect();
                show(
                    ui,
                    json!({"status":"YouTube 解析成功，可以选择画质","ready":true,"busy":false,"formats":formats,"info":format!("{} · {}秒 · {}个可用版本",data["title"].as_str().unwrap_or(""),data["duration"],formats.len()),"log":"默认选最高分辨率，同分辨率优先选择 H.264 便于播放。原始音视频合并，不重新压缩；WebM 视频轨保存为 MKV。"}),
                );
                if self.test_started {
                    self.write_test(
                        json!({"parsed":true,"duration":data["duration"],"formats":formats}),
                    );
                }
                let key = data["formats"][0]["id"].clone();
                self.selected = Some(data);
                if self.test_started {
                    if self.test_download {
                        self.ui(
                            &json!({"action":"download","quality":key}),
                            ui,
                            proxy,
                            busy,
                            downloads,
                        );
                    } else {
                        self.finished_test = true;
                    }
                }
            }
            Err(error) => {
                show(
                    ui,
                    json!({"status":"YouTube 解析失败，可重试","ready":false,"busy":false,"log":error}),
                );
                if self.test_started {
                    self.write_test(json!({"error":error}));
                    self.finished_test = true;
                }
            }
        }
    }
    pub fn finished(&mut self, x: &Value) {
        self.stop();
        if self.test_started && self.test_download {
            self.write_test(x.clone());
            self.finished_test = true;
        }
    }
    pub fn tick(
        &mut self,
        ui: &WebView,
        proxy: &tao::event_loop::EventLoopProxy<Msg>,
        busy: &Arc<AtomicBool>,
        downloads: &PathBuf,
    ) {
        if !self.test_started
            && self.test_url.is_some()
            && self.boot.elapsed() > Duration::from_secs(2)
        {
            self.test_started = true;
            let raw = self.test_url.clone().unwrap();
            self.ui(
                &json!({"action":"parse","url":raw}),
                ui,
                proxy,
                busy,
                downloads,
            );
        }
        if self.login_open
            && Instant::now() >= self.next_login
            && !self.login_pending.swap(true, Ordering::Relaxed)
        {
            self.next_login = Instant::now() + Duration::from_secs(5);
            let p = proxy.clone();
            let pending = self.login_pending.clone();
            browser_cookies::read(&self.browser, "https://www.youtube.com/", move |result| {
                let state = match result {
                    Ok(c) => {
                        if c.iter().any(|c| {
                            ["SID", "SAPISID", "__Secure-3PAPISID"].contains(&c.name.as_str())
                                && !c.value.is_empty()
                        }) {
                            "已保存 YouTube 登录状态；需要时勾选使用登录状态"
                        } else {
                            "未检测到 YouTube 登录，公开视频可先直接解析"
                        }
                    }
                    Err(_) => "登录状态暂无法确认，可在浏览器查看",
                };
                pending.store(false, Ordering::Relaxed);
                let _ = p.send_event(Msg::LoginStatus("youtube", state));
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        for u in [
            "https://www.youtube.com/watch?v=BaW_jenozKc&list=example",
            "https://youtu.be/BaW_jenozKc?t=1",
            "https://www.youtube.com/shorts/BaW_jenozKc",
        ] {
            assert_eq!(video_id(u).as_deref(), Some("BaW_jenozKc"));
        }
        for u in [
            "https://youtube.com.evil.org/watch?v=BaW_jenozKc",
            "http://youtu.be/BaW_jenozKc",
            "https://www.youtube.com/playlist?list=example",
            "https://user@www.youtube.com/watch?v=BaW_jenozKc",
        ] {
            assert!(video_id(u).is_none());
        }
    }
    #[test]
    fn formats() {
        let data = json!({"id":"BaW_jenozKc","duration":10,"formats":[{"format_id":"140","vcodec":"none","acodec":"aac"},{"format_id":"137","width":1920,"height":1080,"fps":30,"vcodec":"avc1","acodec":"none","ext":"mp4","protocol":"https"},{"format_id":"248","width":1920,"height":1080,"fps":30,"vcodec":"vp9","acodec":"none","ext":"webm"},{"format_id":"999","width":3840,"height":2160,"vcodec":"av01","acodec":"none","has_drm":true}]});
        let v = normalize(&data, "BaW_jenozKc").unwrap();
        assert_eq!(v["formats"].as_array().unwrap().len(), 2);
        assert_eq!(v["formats"][0]["id"], "137");
        assert!(!format_key("137/best"));
        assert!(normalize(&data, "wrong").is_err());
    }
    #[test]
    fn progress_reader_survives_non_utf8() {
        let mut c = command("powershell.exe");
        c.args(["-NoProfile","-Command","$s=[Console]::OpenStandardOutput();$s.WriteByte(255);$s.WriteByte(10);[Console]::WriteLine('OK')"]);
        let output = run_tool(c, Duration::from_secs(15), |_| {}).unwrap();
        assert!(output.contains("OK"));
    }
    #[test]
    fn private_job_cleans_cookie_file() {
        let job = Job::new("# Netscape HTTP Cookie File\n").unwrap();
        let cookie = job.cookie_file.clone();
        let dir = job.dir.clone();
        assert!(cookie.exists());
        drop(job);
        assert!(!cookie.exists());
        assert!(!dir.exists());
    }
}
