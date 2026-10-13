use super::*;
use tao::{event_loop::EventLoop, window::Window};

pub fn video_id(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw.trim()).ok()?;
    if !host_allowed(u.as_str(), "douyin.com") {
        return None;
    }
    if let Some(id) = u
        .query_pairs()
        .find(|(k, _)| k == "modal_id")
        .map(|(_, v)| v.into_owned())
    {
        if valid_id(&id) {
            return Some(id);
        }
    }
    let segments: Vec<_> = u.path_segments()?.collect();
    segments
        .windows(2)
        .find(|p| p[0] == "video" && valid_id(p[1]))
        .map(|p| p[1].to_owned())
}
fn valid_id(id: &str) -> bool {
    (15..=22).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}
pub fn parse_url(raw: &str) -> Option<(String, String)> {
    // Share text can contain a short URL amongst Chinese text. Only explicit HTTPS links.
    for token in raw.split_whitespace() {
        let raw = token.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | '，' | '。'));
        if let Some(id) = video_id(raw) {
            return Some((format!("https://www.douyin.com/video/{id}"), id));
        }
        if let Ok(u) = url::Url::parse(raw) {
            if u.scheme() == "https"
                && u.host_str() == Some("v.douyin.com")
                && u.path()
                    .trim_matches('/')
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                && !u.path().trim_matches('/').is_empty()
            {
                return Some((u.to_string(), String::new()));
            }
        }
    }
    None
}
pub fn media_allowed(raw: &str) -> bool {
    [
        "douyinvod.com",
        "douyin.com",
        "bytecdn.cn",
        "bytedance.com",
        "ibytedtos.com",
        "amemv.com",
        "ixigua.com",
        "snssdk.com",
    ]
    .iter()
    .any(|domain| host_allowed(raw, domain))
}
fn navigate_allowed(raw: &str) -> bool {
    raw == "about:blank"
        || host_allowed(raw, "douyin.com")
        || host_allowed(raw, "iesdouyin.com")
        || host_allowed(raw, "bytepassport.com")
}
fn display(ui: &WebView, mut value: Value) {
    value["platform"] = json!("douyin");
    let _ = ui.evaluate_script(&format!("window.update({value});"));
}
fn script(id: &str, request: u64) -> String {
    format!(
        "(()=>{{const expectedId={};const requestId={request};{} }})();",
        json!(id),
        include_str!("douyin-extract.js")
    )
}
pub struct Douyin {
    pub window: Window,
    pub browser: WebView,
    _context: WebContext,
    wanted: String,
    current_url: String,
    selected: Option<Value>,
    request: u64,
    deadline: Option<Instant>,
    next_extract: Instant,
    next_login: Instant,
    login_open: bool,
    pub finished_test: bool,
    test_url: Option<String>,
    test_download: bool,
    test_started: bool,
    diagnosed: bool,
    boot: Instant,
}
impl Douyin {
    pub fn new(events: &EventLoop<Msg>) -> Result<Self, Box<dyn std::error::Error>> {
        let window = WindowBuilder::new()
            .with_title("抖音登录与视频画质 — 下载助手")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(1100., 800.))
            .build(events)?;
        let profile = if std::env::args().any(|a| a == "--isolated-test-profile") {
            std::env::current_dir()?
                .join("verification")
                .join("DouyinTestProfile")
        } else {
            PathBuf::from(std::env::var("LOCALAPPDATA")?)
                .join("BiliDownloader")
                .join("DouyinBrowserProfile")
        };
        std::fs::create_dir_all(&profile)?;
        let mut context = WebContext::new(Some(profile));
        let p = events.create_proxy();
        let navp = p.clone();
        let popup = p.clone();
        let browser = WebViewBuilder::new_with_web_context(&mut context).with_url("about:blank")
            .with_navigation_handler(move |u| { let allowed = navigate_allowed(&u); if !allowed { let _=navp.send_event(Msg::Progress(json!({"platform":"douyin","status":"网页跳转受限","log":format!("暂不支持跳转至 {}",safe_address(&u))}))); } allowed })
            .with_initialization_script(include_str!("douyin-observe.js"))
            .with_new_window_req_handler(move |u, _| { if navigate_allowed(&u) { let _=popup.send_event(Msg::DouyinOpen(u)); } NewWindowResponse::Deny })
            .with_ipc_handler(move |r| { if host_allowed(&r.uri().to_string(), "douyin.com") { if let Ok(v)=serde_json::from_str(r.body()) { let _=p.send_event(Msg::DouyinCapture(v)); } } }).build(&window)?;
        mute_browser(&browser)?;
        let test_url = std::env::args()
            .position(|a| a == "--douyin-smoke-test")
            .and_then(|i| std::env::args().nth(i + 1));
        Ok(Self {
            window,
            browser,
            _context: context,
            wanted: String::new(),
            current_url: String::new(),
            selected: None,
            request: 0,
            deadline: None,
            next_extract: Instant::now(),
            next_login: Instant::now(),
            login_open: false,
            finished_test: false,
            test_url,
            test_download: std::env::args().any(|a| a == "--download-test"),
            test_started: false,
            diagnosed: false,
            boot: Instant::now(),
        })
    }
    pub fn parsing(&self) -> bool {
        self.deadline.is_some()
    }
    pub fn stop(&self) {
        stop_preview(&self.browser, None);
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
                let target = if self.current_url.is_empty() {
                    "https://www.douyin.com/"
                } else {
                    &self.current_url
                };
                let _ = self.browser.load_url(target);
                self.window.set_visible(true);
                self.window.set_focus();
                self.next_login = Instant::now() + Duration::from_secs(3);
            }
            "folder" => {
                let _ = command("explorer.exe").arg(downloads).spawn();
            }
            "platform-selected" => self.stop(),
            "clear" if !busy.load(Ordering::Relaxed) && !self.parsing() => {
                self.selected = None;
                self.wanted.clear();
                self.stop();
            }
            "parse" if !busy.load(Ordering::Relaxed) && !self.parsing() => {
                self.selected = None;
                if let Some((u, id)) = parse_url(v["url"].as_str().unwrap_or("")) {
                    self.wanted = id;
                    self.current_url = u.clone();
                    self.request += 1;
                    self.deadline = Some(Instant::now() + Duration::from_secs(50));
                    self.next_extract = Instant::now() + Duration::from_secs(1);
                    self.diagnosed = false;
                    display(
                        ui,
                        json!({"status":"正在解析抖音视频…","info":"正在读取单条作品的可用画质","ready":false,"busy":true,"formats":[],"log":"网页预览静音。正在等待作品信息；如需要验证，程序会显示浏览器。"}),
                    );
                    if let Err(e) = self.browser.load_url(&u) {
                        self.fail(ui, &format!("无法打开网页：{e}"));
                    }
                } else {
                    display(
                        ui,
                        json!({"status":"请输入抖音单条视频链接","ready":false,"busy":false,"formats":[],"log":"支持 /video/编号、分享短链接，或带 modal_id 的链接。搜索列表、图文和直播暂不支持。"}),
                    );
                }
            }
            "download" if !busy.load(Ordering::Relaxed) && !self.parsing() => {
                if let Some(data) = self.selected.clone() {
                    let key = v["quality"].as_str().unwrap_or("0");
                    let Some(format) = data["formats"]
                        .as_array()
                        .and_then(|a| a.iter().find(|f| f["id"].as_str() == Some(key)))
                        .cloned()
                    else {
                        return;
                    };
                    let raw = format["url"].as_str().unwrap_or("");
                    if !media_allowed(raw) {
                        self.fail(ui, "媒体地址无效，请重新解析。");
                        return;
                    }
                    let cookies = self
                        .browser
                        .cookies_for_url(raw)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|c| !c.value().is_empty())
                        .map(|c| format!("{}={}", c.name(), c.value()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    busy.store(true, Ordering::Relaxed);
                    let p = proxy.clone();
                    let lock = busy.clone();
                    let dir = downloads.clone();
                    display(
                        ui,
                        json!({"status":"开始下载抖音视频","busy":true,"ready":false}),
                    );
                    self.stop();
                    std::thread::spawn(move || {
                        let result = download_video(&data, &format, &cookies, &dir, |mut x| {
                            x["platform"] = json!("douyin");
                            let _ = p.send_event(Msg::Progress(x));
                        });
                        lock.store(false, Ordering::Relaxed);
                        let mut x = match result {
                            Ok(x) => x,
                            Err(e) => json!({"status":"下载失败","log":e,"ready":true}),
                        };
                        x["platform"] = json!("douyin");
                        x["busy"] = json!(false);
                        let _ = p.send_event(Msg::DownloadFinished(x));
                    });
                }
            }
            _ => {}
        }
    }
    fn fail(&mut self, ui: &WebView, error: &str) {
        self.deadline = None;
        self.stop();
        display(
            ui,
            json!({"status":"抖音解析失败，可重试","busy":false,"ready":false,"log":error}),
        );
        if self.test_started {
            self.write_test(json!({"error":error}));
            self.finished_test = true;
        }
    }
    fn write_test(&self, value: Value) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::write(
                exe.with_file_name("douyin-test.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            );
        }
    }
    pub fn captured(
        &mut self,
        data: Value,
        ui: &WebView,
        proxy: &tao::event_loop::EventLoopProxy<Msg>,
        busy: &Arc<AtomicBool>,
        downloads: &PathBuf,
    ) {
        if !self.parsing() || data["requestId"].as_u64() != Some(self.request) {
            return;
        }
        if data["waiting"] == true {
            return;
        }
        let id = data["id"].as_str().unwrap_or("");
        if !valid_id(id) || (!self.wanted.is_empty() && self.wanted != id) {
            return;
        }
        if let Some(error) = data["error"].as_str() {
            self.fail(ui, error);
            return;
        }
        let Some(formats) = data["formats"].as_array() else {
            return;
        };
        if formats.is_empty()
            || formats.len() > 30
            || !formats
                .iter()
                .all(|f| f["url"].as_str().is_some_and(media_allowed))
            || !data["duration"]
                .as_f64()
                .is_some_and(|d| d.is_finite() && d > 0.)
        {
            self.fail(ui, "未取得完整的作品信息，请在浏览器确认视频可播放后重试。");
            return;
        }
        self.wanted = id.to_owned();
        self.deadline = None;
        self.stop();
        let public: Vec<_> = formats
            .iter()
            .map(|f| json!({"id":f["id"],"label":f["label"]}))
            .collect();
        display(
            ui,
            json!({"status":"抖音解析成功，可以选择画质","ready":true,"busy":false,"formats":public,"info":format!("{} · {:.1}秒 · {}个可用版本",data["title"].as_str().unwrap_or("抖音视频"),data["duration"].as_f64().unwrap(),formats.len()),"log":if data["source"]=="playback"{"本次只获取到当前播放画质，不代表作品的全部版本。要尝试其他画质，可显示浏览器、登录并调整播放器画质后重新解析。"}else{"默认选择最高可用画质。请选择下拉框中的画质，再点击“下载所选画质”。链接过期时重新解析。"}}),
        );
        if self.test_started {
            self.write_test(json!({"parsed":true,"duration":data["duration"],"formats":public,"muted":browser_is_muted(&self.browser).unwrap_or(false)}));
        }
        self.selected = Some(data);
        if self.test_started {
            if self.test_download {
                self.ui(
                    &json!({"action":"download","quality":"0"}),
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
            let url = self.test_url.clone().unwrap();
            self.ui(
                &json!({"action":"parse","url":url}),
                ui,
                proxy,
                busy,
                downloads,
            );
        }
        if self.parsing() && Instant::now() >= self.next_extract {
            self.next_extract = Instant::now() + Duration::from_secs(2);
            let _ = self
                .browser
                .evaluate_script(&script(&self.wanted, self.request));
        }
        if self.parsing()
            && !self.diagnosed
            && self.boot.elapsed() > Duration::from_secs(12)
            && self.test_started
        {
            self.diagnosed = true;
            let _=self.browser.evaluate_script_with_callback("JSON.stringify({path:location.pathname,ready:document.readyState,title:document.title,text:document.body?.innerText.slice(0,600),captured:window.__douyinCapture?.items.size,video:[...document.querySelectorAll('video')].map(v=>({width:v.videoWidth,height:v.videoHeight,duration:v.duration,src:v.currentSrc?.startsWith('blob:')?'blob':v.currentSrc?new URL(v.currentSrc).hostname:''})),scripts:[...document.querySelectorAll('script[id]')].map(x=>x.id)})",|s|{if let Ok(exe)=std::env::current_exe(){let _=std::fs::write(exe.with_file_name("douyin-page-test.json"),s);}});
        }
        if self.parsing() && self.deadline.is_some_and(|t| Instant::now() >= t) {
            self.fail(ui,"网页未返回完整的视频信息。请点击“登录 / 显示浏览器”，完成登录或验证，确认视频可以播放后重新解析。");
            self.window.set_visible(true);
            self.window.set_focus();
        }
        if (self.login_open || self.parsing()) && Instant::now() >= self.next_login {
            self.next_login = Instant::now() + Duration::from_secs(5);
            let state = match self.browser.cookies_for_url("https://www.douyin.com/") {
                Ok(cookies) => {
                    if cookies.iter().any(|c| {
                        ["sessionid", "sessionid_ss", "sid_guard"].contains(&c.name())
                            && !c.value().is_empty()
                    }) {
                        "已保存抖音登录状态；失效时请重新登录"
                    } else {
                        "尚未检测到抖音登录，请在浏览器确认"
                    }
                }
                Err(_) => "暂无法确认抖音登录状态，可在浏览器查看",
            };
            let _ = ui.evaluate_script(&format!("window.setLogin({},'douyin');", json!(state)));
        }
    }
}
fn download_video(
    data: &Value,
    format: &Value,
    cookies: &str,
    dir: &PathBuf,
    report: impl Fn(Value),
) -> Result<Value, String> {
    let id = data["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or("缺少作品编号")?;
    let raw = format["url"]
        .as_str()
        .filter(|u| media_allowed(u))
        .ok_or("媒体地址无效")?;
    let title = data["title"]
        .as_str()
        .unwrap_or("抖音视频")
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(70)
        .collect::<String>();
    let base = format!("{} [DY{id}]", title.trim_end_matches([' ', '.']));
    let mut out = dir.join(format!("{base}.mp4"));
    let mut n = 1;
    while out.exists() {
        out = dir.join(format!("{base} ({n}).mp4"));
        n += 1;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let temp = dir.join(format!(".douyin-{id}-{stamp}.part"));
    // Credentials remain in memory and go via stdin, not command-line arguments or a cookies file.
    let mut c = command("curl.exe");
    c.args([
        "--fail",
        "--silent",
        "--show-error",
        "--connect-timeout",
        "20",
        "--max-time",
        "1800",
        "--retry",
        "2",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-redirs",
        "5",
        "-A",
        data["userAgent"].as_str().unwrap_or("Mozilla/5.0"),
        "-e",
        &format!("https://www.douyin.com/video/{id}"),
        "-o",
    ])
    .arg(&temp)
    .arg(raw)
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    if cookies.is_empty() {
        c.arg("--location");
    } else {
        c.args(["--config", "-"]).stdin(Stdio::piped());
    }
    let mut child = c.spawn().map_err(|e| format!("无法启动下载：{e}"))?;
    if !cookies.is_empty() {
        use std::io::Write;
        if let Some(mut input) = child.stdin.take() {
            let _ = writeln!(
                input,
                "cookie = {}",
                serde_json::to_string(cookies).unwrap()
            );
        }
    }
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("媒体下载失败；可能链接已过期。请重新解析后重试。".into());
            }
            break;
        }
        let size = std::fs::metadata(&temp).map(|m| m.len()).unwrap_or(0);
        report(
            json!({"status":format!("正在下载抖音视频 · {:.1} MB",size as f64/1048576.),"busy":true}),
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    report(json!({"status":"正在核验抖音视频…","busy":true}));
    let probe = command(&tool("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,size:stream=codec_type,codec_name,width,height",
            "-of",
            "json",
        ])
        .arg(&temp)
        .output()
        .map_err(|_| "未找到 FFprobe，请将 ffprobe.exe 放在程序旁边。")?;
    if !probe.status.success() {
        return Err(
            "下载内容无法识别为视频，可能网页资源已失效，请重新解析。暂存文件已保留。".into(),
        );
    }
    let info: Value = serde_json::from_slice(&probe.stdout).map_err(|e| e.to_string())?;
    let streams = info["streams"].as_array().ok_or("未找到媒体流")?;
    let video = streams
        .iter()
        .find(|v| v["codec_type"] == "video")
        .ok_or("未找到视频流")?;
    let duration = info["format"]["duration"]
        .as_str()
        .and_then(|v| v.parse::<f64>().ok())
        .ok_or("无法核验时长")?;
    let expected = data["duration"].as_f64().ok_or("源时长无效")?;
    if !duration.is_finite() || duration <= 0. || (duration - expected).abs() > 3. {
        return Err("下载文件与源视频时长不一致，暂存文件已保留，请重新解析。".into());
    }
    let width = video["width"].as_u64().unwrap_or(0);
    let height = video["height"].as_u64().unwrap_or(0);
    if width == 0 || height == 0 {
        return Err("视频分辨率无效".into());
    }
    let size = std::fs::metadata(&temp).map_err(|e| e.to_string())?.len();
    std::fs::rename(&temp, &out).map_err(|e| e.to_string())?;
    Ok(
        json!({"status":"下载完成 ✓","ready":true,"info":format!("{width}×{height} · {duration:.1}秒 · {:.1}MB",size as f64/1048576.),"log":format!("保存位置：{}\n分辨率、媒体流及完整时长核验通过；直接保存，未重新压缩。{}",out.display(),if streams.iter().any(|s|s["codec_type"]=="audio"){"保留原有声音。"}else{"源文件无音轨。"})}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links() {
        assert_eq!(
            video_id("https://www.douyin.com/jingxuan/search/example?modal_id=7000000000000000001")
                .as_deref(),
            Some("7000000000000000001")
        );
        assert_eq!(
            parse_url("分享 https://v.douyin.com/AbC_dE/ 复制打开")
                .unwrap()
                .1,
            ""
        );
        assert!(parse_url("https://www.douyin.com/search/example").is_none());
        assert!(parse_url("https://evil.douyin.com.evil.org/video/7000000000000000001").is_none());
        assert!(parse_url("https://www.douyin.com/note/7000000000000000001").is_none());
    }
    #[test]
    fn media_domains() {
        assert!(media_allowed("https://v3.douyinvod.com/video/test"));
        assert!(!media_allowed("https://douyinvod.com.evil.org/video"));
        assert!(!media_allowed("http://v3.douyinvod.com/video"));
        assert!(!media_allowed("https://cdn.bilivideo.com/video"));
    }
}
