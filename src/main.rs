#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod browser_cookies;
mod douyin;
mod download_process;
mod transcription;
mod youtube;
use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::{cell::RefCell, rc::Rc};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
use windows_core::Interface;
use wry::{
    NewWindowResponse, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows,
    WebViewExtWindows,
};
#[derive(Clone, Debug)]
enum Msg {
    Ui(Value),
    Transcription(Value),
    YouTubeParsed(u64, Result<Value, String>),
    YouTubeOpen(String),
    DouyinCapture(Value),
    DouyinOpen(String),
    Capture(Value),
    Progress(Value),
    DownloadFinished(Value),
    MediaStarted(bool),
    MediaStopped(bool),
    PopupClosed,
    PopupTest(bool),
    Loaded(String, bool),
    LoginStatus(&'static str, &'static str),
}
fn host_allowed(s: &str, domain: &str) -> bool {
    url::Url::parse(s)
        .map(|u| {
            u.scheme() == "https"
                && u.host_str()
                    .map(|h| h == domain || h.ends_with(&format!(".{domain}")))
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}
fn media_url_allowed(s: &str) -> bool {
    host_allowed(s, "bilivideo.com") || host_allowed(s, "bilivideo.cn")
}
fn auth_navigation(s: &str) -> bool {
    s == "about:blank"
        || s.starts_with("about:blank#")
        || host_allowed(s, "bilibili.com")
        || host_allowed(s, "qq.com")
}
fn safe_address(s: &str) -> String {
    url::Url::parse(s)
        .map(|u| {
            format!(
                "{}://{}{}",
                u.scheme(),
                u.host_str().unwrap_or(""),
                u.path()
            )
        })
        .unwrap_or_else(|_| "无效地址".into())
}
fn auth_log(source: &str, s: &str) {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(exe.with_file_name("login-diagnostic.txt"))
        {
            use std::io::Write;
            let _ = writeln!(f, "{} {}", source, safe_address(s));
        }
    }
}
fn bvid(s: &str) -> Option<String> {
    let raw = s.trim();
    if raw.starts_with("BV") && raw.len() == 12 && raw.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(raw.into());
    }
    if !host_allowed(raw, "bilibili.com") {
        return None;
    }
    url::Url::parse(raw)
        .ok()?
        .path_segments()?
        .find(|x| {
            x.starts_with("BV") && x.len() == 12 && x.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map(str::to_owned)
}
fn command(s: &str) -> Command {
    let mut c = Command::new(s);
    c.creation_flags(0x08000000);
    c
}
fn tool(name: &str) -> String {
    let p = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!("{name}.exe"));
    if p.exists() {
        p.to_string_lossy().into()
    } else {
        name.into()
    }
}
fn mute_browser(view: &WebView) -> windows_core::Result<()> {
    let native = view.webview().cast::<ICoreWebView2_8>()?;
    unsafe { native.SetIsMuted(true) }
}
fn browser_is_muted(view: &WebView) -> windows_core::Result<bool> {
    let native = view.webview().cast::<ICoreWebView2_8>()?;
    let mut muted = windows_core::BOOL::default();
    unsafe {
        native.IsMuted(&mut muted)?;
    }
    Ok(muted.as_bool())
}
fn stop_preview(browser: &WebView, popup: Option<&WebView>) {
    let _ = browser.evaluate_script(include_str!("stop-media.js"));
    if let Some(view) = popup {
        let _ = view.evaluate_script(include_str!("stop-media.js"));
    }
}
fn extract_script(bv: &str, id: u64) -> String {
    format!(
        "(()=>{{const expectedBv={};const requestId={};{} }})();",
        json!(bv),
        id,
        include_str!("extract.js")
    )
}
fn main() {
    if let Err(error) = run() {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::write(
                exe.with_file_name("startup-diagnostic.txt"),
                format!("程序启动失败：{error}"),
            );
        }
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(input) = std::env::args().nth(1).filter(|x| x == "--verify-download") {
        let _ = input;
        let file = std::env::args().nth(2).ok_or("Missing input")?;
        let data: Value = serde_json::from_slice(&std::fs::read(file)?)?;
        let dir = std::env::current_dir()?.join("verification");
        std::fs::create_dir_all(&dir)?;
        let result = download(&data, &dir, &AtomicBool::new(false), |_| {})
            .map_err(std::io::Error::other)?;
        std::fs::write(dir.join("result.json"), serde_json::to_vec_pretty(&result)?)?;
        return Ok(());
    }
    let parse_test = std::env::args()
        .nth(1)
        .filter(|s| s == "--parse-smoke-test")
        .and_then(|_| std::env::args().nth(2));
    let delayed_test = std::env::args().any(|x| x == "--delayed-navigation");
    let transcript_test = std::env::args()
        .position(|x| x == "--transcription-test")
        .and_then(|i| std::env::args().nth(i + 1));
    let media_smoke = std::env::args().any(|x| x == "--media-smoke-test");
    let events = EventLoopBuilder::<Msg>::with_user_event().build();
    let proxy = events.create_proxy();
    let window = WindowBuilder::new()
        .with_title("视频下载助手 v0.4.0 试用版")
        .with_visible(
            !media_smoke
                && transcript_test.is_none()
                && parse_test.is_none()
                && !std::env::args().any(|a| {
                    a == "--douyin-smoke-test"
                        || a == "--youtube-smoke-test"
                        || a == "--ui-review-test"
                }),
        )
        .with_inner_size(LogicalSize::new(900., 730.))
        .build(&events)?;
    let browser_window = WindowBuilder::new()
        .with_title("B站登录与视频画质 — 下载助手")
        .with_visible(false)
        .with_inner_size(LogicalSize::new(1100., 800.))
        .build(&events)?;
    let profile = if std::env::args().any(|a| a == "--isolated-test-profile") {
        std::env::current_dir()?
            .join("verification")
            .join("BiliTestProfile")
    } else {
        PathBuf::from(std::env::var("LOCALAPPDATA")?)
            .join("BiliDownloader")
            .join("BrowserProfile")
    };
    std::fs::create_dir_all(&profile)?;
    let mut context = WebContext::new(Some(profile));
    let p = proxy.clone();
    let ui = WebViewBuilder::new()
        .with_html(include_str!("ui.html"))
        .with_navigation_handler(|s| {
            s == "about:blank" || s.starts_with("data:text/html;charset=utf-8;base64,")
        })
        .with_ipc_handler(move |r| {
            if let Ok(v) = serde_json::from_str(r.body()) {
                let _ = p.send_event(Msg::Ui(v));
            }
        })
        .build(&window)?;
    let return_smoke = std::env::args().any(|x| x == "--auth-return-test");
    let oauth_smoke = std::env::args().any(|x| x == "--oauth-smoke-test");
    let popup_window = Rc::new(
        WindowBuilder::new()
            .with_title("QQ登录授权 — B站下载助手")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(850., 700.))
            .build(&events)?,
    );
    let popup_view: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let popup_host = popup_window.clone();
    let popup_store = popup_view.clone();
    let popup_proxy = proxy.clone();
    let nav_proxy = proxy.clone();
    let load_proxy = proxy.clone();
    let p = proxy.clone();
    let browser=WebViewBuilder::new_with_web_context(&mut context).with_url(if media_smoke{"about:blank"}else{"https://www.bilibili.com/"}).with_navigation_handler(move|s|{let allowed=auth_navigation(&s);if !allowed{let _=nav_proxy.send_event(Msg::Progress(json!({"status":"登录跳转受限","log":format!("未支持的登录跳转：{}。请重新打开登录页面。",safe_address(&s))})));}allowed})
 .with_on_page_load_handler(move|ev,u|{if matches!(ev,wry::PageLoadEvent::Finished){let _=load_proxy.send_event(Msg::Loaded(u,false));}})
 .with_initialization_script("window.close=()=>window.ipc.postMessage(JSON.stringify({action:'auth-return'}));")
 .with_new_window_req_handler(move|url,features|{
   if !auth_navigation(&url){let _=popup_proxy.send_event(Msg::Progress(json!({"status":"已拦截无关网站弹窗","log":"授权窗口仅允许B站和QQ的HTTPS页面。"})));return NewWindowResponse::Deny;}
   let p=popup_proxy.clone();let lp=popup_proxy.clone();
   let result=WebViewBuilder::new().with_environment(features.opener.environment)
     .with_navigation_handler(|s|auth_navigation(&s))
     .with_on_page_load_handler(move|ev,u|{if matches!(ev,wry::PageLoadEvent::Finished){let _=lp.send_event(Msg::Loaded(u,true));}})
     .with_initialization_script("window.close=()=>window.ipc.postMessage('close');")
     .with_ipc_handler(move|r|{if r.body()=="close"{let _=p.send_event(Msg::PopupClosed);}}).build(popup_host.as_ref());
   match result{
     Ok(view)=>{if mute_browser(&view).is_err(){let _=popup_proxy.send_event(Msg::Progress(json!({"status":"无法将授权浏览器设为静音","log":"请更新Edge WebView2 Runtime后重试。"})));return NewWindowResponse::Deny;}let native=view.webview();*popup_store.borrow_mut()=Some(view);if !oauth_smoke&&!return_smoke{popup_host.set_visible(true);popup_host.set_focus();}NewWindowResponse::Create{webview:native}},
     Err(_)=>{let _=popup_proxy.send_event(Msg::Progress(json!({"status":"无法打开QQ授权窗口","log":"请关闭并重新打开程序后重试。"})));NewWindowResponse::Deny}
   }
 })
 .with_ipc_handler(move|r|{if host_allowed(&r.uri().to_string(),"bilibili.com"){if let Ok(v)=serde_json::from_str(r.body()){let _=p.send_event(Msg::Capture(v));}}}).build(&browser_window)?;

    mute_browser(&browser)?;
    let mut wanted = String::new();
    let mut current_url = String::new();
    let mut selected: Option<Value> = None;
    let busy = Arc::new(AtomicBool::new(false));
    let mut dy = douyin::Douyin::new(&events)?;
    let mut yt = youtube::YouTube::new(&events)?;
    let mut transcript = transcription::Transcription::default();
    let mut transcript_result: Option<Value> = None;
    let downloads = PathBuf::from(std::env::var("USERPROFILE")?).join("Downloads");
    std::fs::create_dir_all(&downloads)?;
    let mut pending: Option<Instant> = None;
    let mut parse_deadline: Option<Instant> = None;
    let mut parse_request = 0u64;
    let mut parse_attempts = 0u8;
    let mut delayed_navigation: Option<(Instant, String)> = None;
    let mut login_probe: Option<Instant> = if return_smoke {
        None
    } else {
        Some(Instant::now() + Duration::from_secs(4))
    };
    let login_state_test = std::env::args().any(|x| x == "--login-state-test") || return_smoke;
    let mut login_open = login_state_test;
    if login_state_test && !return_smoke {
        login_probe = Some(Instant::now() + Duration::from_secs(4));
    }
    let smoke = std::env::args().any(|x| x == "--smoke-test");
    let boot = Instant::now();
    let mut inspected = false;
    let mut oauth_opened = false;
    let mut oauth_checked = false;
    let mut media_injected = false;
    let mut media_phase = 0u8;
    let mut bili_cancel: Option<Arc<AtomicBool>> = None;
    let mut stopping = false;
    events.run(move|event,_,flow|{
 *flow=ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(300));
 let display=|x:Value|{let _=ui.evaluate_script(&format!("window.update({});",x));};
 match event{
 Event::WindowEvent{window_id,event:WindowEvent::CloseRequested,..}=>{if window_id==window.id(){transcript.stop();*flow=ControlFlow::Exit}else if window_id==yt.window.id(){yt.close_browser()}else if window_id==dy.window.id(){dy.close_browser()}else if window_id==popup_window.id(){popup_window.set_visible(false)}else{login_open=false;login_probe=None;stop_preview(&browser,None);browser_window.set_visible(false)}},
 Event::UserEvent(Msg::Ui(v)) if v["platform"]=="transcribe"=>{if v["action"]=="platform-selected"{yt.stop();dy.stop();stop_preview(&browser,popup_view.borrow().as_ref());}transcript.ui(&v,&ui,&proxy,&busy);},
 Event::UserEvent(Msg::Transcription(v))=>{if v["output"].is_string(){transcript_result=Some(v.clone());}let finished=v["busy"]==false;transcript.event(v.clone(),&ui);if finished&&transcript_test.is_some(){let report=json!({"success":transcript_result.as_ref().and_then(|r|r["output"].as_str()).is_some_and(|p|std::path::Path::new(p).is_file()),"result":transcript_result,"final":v});let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("transcription-test.json"),serde_json::to_vec_pretty(&report).unwrap());*flow=ControlFlow::Exit;}},
 Event::UserEvent(Msg::Ui(v)) if v["action"]=="stop"=>{let requested=match v["platform"].as_str(){Some("youtube")=>yt.cancel_download(),Some("douyin")=>dy.cancel_download(),_=>{if let Some(cancel)=&bili_cancel{cancel.store(true,Ordering::Relaxed);true}else{false}}};if requested{stopping=true;display(json!({"platform":v["platform"],"status":"正在停止下载…","stopping":true}));}},
 Event::UserEvent(Msg::Ui(v)) if v["platform"]=="youtube"=>{if v["action"]=="platform-selected"{dy.stop();stop_preview(&browser,popup_view.borrow().as_ref());}if parse_deadline.is_none()&&!dy.parsing(){yt.ui(&v,&ui,&proxy,&busy,&downloads);}},
 Event::UserEvent(Msg::YouTubeParsed(request,result))=>yt.parsed(request,result,&ui,&proxy,&busy,&downloads),
 Event::UserEvent(Msg::YouTubeOpen(url))=>{let _=yt.browser.load_url(&url);yt.window.set_visible(true);yt.window.set_focus();},
 Event::UserEvent(Msg::Ui(v)) if v["platform"]=="douyin"=>{if v["action"]=="platform-selected"{yt.stop();stop_preview(&browser,popup_view.borrow().as_ref());}if parse_deadline.is_none(){dy.ui(&v,&ui,&proxy,&busy,&downloads);}},
 Event::UserEvent(Msg::DouyinCapture(v))=>dy.captured(v,&ui,&proxy,&busy,&downloads),
 Event::UserEvent(Msg::DouyinOpen(u))=>{let _=dy.browser.load_url(&u);dy.window.set_visible(true);dy.window.set_focus();},
 Event::UserEvent(Msg::Ui(v))=>match v["action"].as_str().unwrap_or(""){
 "ui-ready"=>{if let Some(ref bv)=parse_test{let _=proxy.send_event(Msg::Ui(json!({"action":"parse","url":bv})));}if smoke{let valid=v["buttons"].as_u64()==Some(17)&&v["tabs"].as_u64()==Some(4)&&v["transcription"].as_bool()==Some(true)&&v["input"].as_bool()==Some(true)&&v["visible"].as_bool()==Some(true)&&v["update"].as_bool()==Some(true);let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("smoke-test.txt"),if valid{"PASS: four tabs, local transcription controls, URL input, JavaScript and IPC ready"}else{"FAIL: HTML readiness check failed"});*flow=ControlFlow::Exit;}else if let Some(ref path)=transcript_test{transcript.set_test_file(PathBuf::from(path),&ui);let _=ui.evaluate_script("switchPlatform('transcribe');");let _=proxy.send_event(Msg::Ui(json!({"platform":"transcribe","action":"transcribe-start","language":"zh"})));}},
 "login"=>{login_open=true;let target=if current_url.is_empty(){"https://www.bilibili.com/"}else{&current_url};let _=browser.load_url(target);login_probe=Some(Instant::now()+Duration::from_secs(3));browser_window.set_visible(true);browser_window.set_focus();},
 "platform-selected"=>{dy.stop();yt.stop();},
 "clear" if !busy.load(Ordering::Relaxed)&&parse_deadline.is_none()=>{selected=None;stop_preview(&browser,None);},
 "folder"=>{let _=command("explorer.exe").arg(&downloads).spawn();},
 "parse" if !busy.load(Ordering::Relaxed)&&!dy.parsing()&&parse_deadline.is_none()=>{if let Some(bv)=bvid(v["url"].as_str().unwrap_or("")){selected=None;wanted=bv;parse_request+=1;parse_attempts=0;parse_deadline=Some(Instant::now()+Duration::from_secs(35));let u=format!("https://www.bilibili.com/video/{wanted}/");current_url=u.clone();if delayed_test{delayed_navigation=Some((Instant::now()+Duration::from_secs(6),u));}else{let _=browser.load_url(&u);}pending=Some(Instant::now()+Duration::from_secs(1));display(json!({"status":"正在解析…","info":"正在加载视频页面","log":"正在等待页面就绪；若网络请求失败，会显示具体原因。","ready":false,"busy":true}));}else{display(json!({"status":"请输入有效的 bilibili.com 视频网址或BV编号"}));}},
 "download" if !busy.load(Ordering::Relaxed)=>{if let Some(data)=selected.clone(){busy.store(true,Ordering::Relaxed);let p=proxy.clone();let dir=downloads.clone();let lock=busy.clone();let cancel=Arc::new(AtomicBool::new(false));bili_cancel=Some(cancel.clone());display(json!({"status":"开始下载","busy":true,"downloading":true,"stopping":false}));std::thread::spawn(move||{let result=download(&data,&dir,&cancel,|x|{let _=p.send_event(Msg::Progress(x));});lock.store(false,Ordering::Relaxed);let x=match result{Ok(x)=>x,Err(e)=>download_process::result(e)};let _=p.send_event(Msg::DownloadFinished(x));});}},_=>{}},
 Event::UserEvent(Msg::Capture(v))=>{
 if v["action"]=="auth-return"{let _=proxy.send_event(Msg::PopupClosed);}
 else if v["action"]=="auth-status"{let state=v["state"].as_str().unwrap_or("error");if login_state_test&&(!return_smoke||oauth_checked){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name(if return_smoke{"auth-return-test.txt"}else{"login-state-test.txt"}),format!("state={} blank={}",state,v["blank"]));*flow=ControlFlow::Exit;}let _=ui.evaluate_script(&format!("window.setLogin({});",json!(match state{"logged-in"=>"已登录B站","logged-out"=>"尚未登录B站",_=>"登录状态检测失败，可稍后重试"})));if state=="logged-in"&&v["blank"].as_bool()==Some(true){let target=if current_url.is_empty(){"https://www.bilibili.com/"}else{&current_url};let _=browser.load_url(target);}if login_open{login_probe=Some(Instant::now()+Duration::from_secs(5));}}
 else if v["bv"].as_str()==Some(&wanted)&&v["requestId"].as_u64()==Some(parse_request)&&parse_deadline.is_some()&&!busy.load(Ordering::Relaxed){
   if v["waiting"].as_bool()==Some(true){pending=Some(Instant::now()+Duration::from_secs(1));}
   else if v["started"].as_bool()==Some(true){parse_attempts+=1;pending=Some(Instant::now()+Duration::from_secs(16));}
   else if let Some(e)=v["error"].as_str(){
     if parse_attempts<2{pending=Some(Instant::now()+Duration::from_secs(2));display(json!({"status":"正在重试解析…","log":e,"busy":true}));}
     else{parse_deadline=None;pending=None;display(json!({"status":"解析失败，可重试","log":e,"busy":false}));if parse_test.is_some(){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-smoke-test.json"),serde_json::to_vec_pretty(&json!({"error":e})).unwrap());*flow=ControlFlow::Exit;}}
   }else if media_url_allowed(v["video"].as_str().unwrap_or(""))&&media_url_allowed(v["audio"].as_str().unwrap_or("")){
     parse_deadline=None;pending=None;display(json!({"status":"解析成功，可以下载","info":format!("{} · {} · {}×{} · 约{}秒",v["title"].as_str().unwrap_or(""),v["quality"].as_str().unwrap_or(""),v["width"],v["height"],v["duration"]),"ready":true,"busy":false}));
     if parse_test.is_some(){if std::env::args().any(|x|x=="--verify-media"){let _=std::fs::write(std::env::current_dir().unwrap().join("verification-input-latest.json"),serde_json::to_vec(&v).unwrap());}let result=json!({"title":v["title"],"width":v["width"],"height":v["height"],"quality":v["quality"],"duration":v["duration"],"muted":browser_is_muted(&browser).unwrap_or(false),"delayed":delayed_test});let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-smoke-test.json"),serde_json::to_vec_pretty(&result).unwrap());*flow=ControlFlow::Exit;}
     selected=Some(v);
   }else{parse_deadline=None;pending=None;display(json!({"status":"解析结果不完整","log":"未取得有效音视频地址，请重试。","busy":false}));if parse_test.is_some(){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-smoke-test.json"),"{\"error\":\"invalid media URL\"}");*flow=ControlFlow::Exit;}}
 }},
 Event::UserEvent(Msg::LoginStatus(platform,state))=>{let _=ui.evaluate_script(&format!("window.setLogin({},{});",json!(state),json!(platform)));},
 Event::UserEvent(Msg::Progress(x))=>{if !stopping{display(x);}},
 Event::UserEvent(Msg::DownloadFinished(mut x))=>{stopping=false;x["busy"]=json!(false);x["downloading"]=json!(false);x["stopping"]=json!(false);if x["platform"]=="youtube"{yt.finished(&x);}else if x["platform"]=="douyin"{dy.finished(&x);}else{bili_cancel=None;stop_preview(&browser,popup_view.borrow().as_ref());}display(x);},
 Event::UserEvent(Msg::MediaStarted(ok))=>{if !ok{let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("media-smoke-test.txt"),"FAIL: test media did not play");*flow=ControlFlow::Exit;}else{let _=proxy.send_event(Msg::DownloadFinished(json!({"status":"下载完成 ✓","ready":true})));}},
 Event::UserEvent(Msg::MediaStopped(paused))=>{let muted=browser_is_muted(&browser).unwrap_or(false);let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("media-smoke-test.txt"),if paused&&muted{"PASS: native browser muted; playing media paused after DownloadFinished"}else{"FAIL: mute or pause check failed"});*flow=ControlFlow::Exit;},
 Event::UserEvent(Msg::PopupClosed)=>{popup_window.set_visible(false);if !oauth_smoke{let target=if current_url.is_empty(){"https://www.bilibili.com/"}else{&current_url};let _=browser.load_url(target);login_probe=Some(Instant::now()+Duration::from_secs(3));browser_window.set_visible(true);browser_window.set_focus();}},
 Event::UserEvent(Msg::Loaded(u,popup))=>{auth_log(if popup{"popup"}else{"browser"},&u);if !popup&&parse_deadline.is_some()&&bvid(&u).as_deref()==Some(&wanted){pending=Some(Instant::now()+Duration::from_millis(250));}if !popup&&login_open&&(!return_smoke||oauth_checked){if u=="about:blank"||u.starts_with("about:blank#"){let _=proxy.send_event(Msg::PopupClosed);}else if host_allowed(&u,"bilibili.com"){login_probe=Some(Instant::now()+Duration::from_secs(2));}}},
 Event::UserEvent(Msg::PopupTest(ok))=>{let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("oauth-smoke-test.txt"),if ok{"PASS: popup created in shared environment; opener and callback work"}else{"FAIL: popup opener callback failed"});*flow=ControlFlow::Exit;},
 Event::MainEventsCleared=>{
 yt.tick(&ui,&proxy,&busy,&downloads);if yt.finished_test{*flow=ControlFlow::Exit;}
 dy.tick(&ui,&proxy,&busy,&downloads);if dy.finished_test{*flow=ControlFlow::Exit;}
 if delayed_navigation.as_ref().is_some_and(|(t,_)|Instant::now()>=*t){if let Some((_,u))=delayed_navigation.take(){let _=browser.load_url(&u);}}
 if parse_deadline.is_some_and(|t|Instant::now()>=t){parse_deadline=None;pending=None;display(json!({"status":"解析超时，可重试","log":"视频页面或接口未及时响应，请检查网络，或显示浏览器确认网页状态后重试。","busy":false}));if parse_test.is_some(){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-smoke-test.json"),"{\"error\":\"parse timeout\"}");*flow=ControlFlow::Exit;}}

 if parse_test.is_some()&&!inspected&&boot.elapsed()>Duration::from_secs(10){inspected=true;let _=browser.evaluate_script_with_callback("JSON.stringify({path:location.pathname,ready:document.readyState,ipc:typeof window.ipc,media:document.querySelectorAll('video').length,text:document.body?.innerText.slice(0,180)})",|s|{let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-page-test.json"),s);});}
 if parse_test.is_some()&&boot.elapsed()>Duration::from_secs(40){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("parse-smoke-test.json"),"{\"error\":\"parse timeout\"}");*flow=ControlFlow::Exit;}

 if media_smoke&&!media_injected&&boot.elapsed()>Duration::from_secs(2){media_injected=true;let _=browser.evaluate_script(include_str!("media-test.js"));}
 if media_smoke&&media_injected&&media_phase==0&&boot.elapsed()>Duration::from_secs(4){media_phase=1;let p=proxy.clone();let _=browser.evaluate_script_with_callback("!!window.__testMedia && !window.__testMedia.paused && window.__testMedia.currentTime>0",move|s|{let _=p.send_event(Msg::MediaStarted(s=="true"));});}
 if media_smoke&&media_phase==1&&boot.elapsed()>Duration::from_secs(6){media_phase=2;let p=proxy.clone();let _=browser.evaluate_script_with_callback("!!window.__testMedia && window.__testMedia.paused",move|s|{let _=p.send_event(Msg::MediaStopped(s=="true"));});}
 if media_smoke&&boot.elapsed()>Duration::from_secs(20){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("media-smoke-test.txt"),"FAIL: media test timeout");*flow=ControlFlow::Exit;}

 if login_probe.is_some_and(|t|Instant::now()>=t){login_probe=None;let _=browser.evaluate_script(include_str!("login-status.js"));}

 if (oauth_smoke||return_smoke)&&!oauth_opened&&boot.elapsed()>Duration::from_secs(3){oauth_opened=true;let _=browser.evaluate_script("window.__oauthResult=0;window.__oauthPopup=window.open('about:blank','qq-auth-test');");}
 if return_smoke&&!oauth_checked&&boot.elapsed()>Duration::from_secs(6)&&popup_view.borrow().is_some(){oauth_checked=true;let _=popup_view.borrow().as_ref().unwrap().evaluate_script("window.close();");}
 if oauth_smoke&&!oauth_checked&&boot.elapsed()>Duration::from_secs(6)&&popup_view.borrow().is_some(){oauth_checked=true;let p=proxy.clone();let _=popup_view.borrow().as_ref().unwrap().evaluate_script_with_callback("(()=>{if(!window.opener)return 'FAIL';window.opener.__oauthResult=42;return window.opener.__oauthResult===42?'PASS':'FAIL';})()",move|s|{let _=p.send_event(Msg::PopupTest(s.contains("PASS")));});}
 if login_state_test&&boot.elapsed()>Duration::from_secs(25){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("login-state-test.txt"),"FAIL: login state timeout");*flow=ControlFlow::Exit;}
 if oauth_smoke&&boot.elapsed()>Duration::from_secs(25){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("oauth-smoke-test.txt"),"FAIL: popup timeout");*flow=ControlFlow::Exit;}
if smoke&&!inspected&&boot.elapsed()>Duration::from_secs(3){inspected=true;let _=ui.evaluate_script_with_callback("JSON.stringify({url:location.href,html:document.documentElement.outerHTML,ipc:typeof window.ipc})",|s|{let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("ui-diagnostic.txt"),s);});}if smoke&&boot.elapsed()>Duration::from_secs(25){let _=std::fs::write(std::env::current_exe().unwrap().with_file_name("smoke-test.txt"),"FAIL: HTML load timeout");*flow=ControlFlow::Exit;}else if pending.is_some_and(|t|Instant::now()>=t){pending=Some(Instant::now()+Duration::from_secs(3));if browser.evaluate_script(&extract_script(&wanted,parse_request)).is_err(){parse_deadline=None;pending=None;display(json!({"status":"解析脚本启动失败","log":"请重新打开浏览器后重试。","busy":false}));}}},_=>{}
 }
 });
}
fn download(
    data: &Value,
    dir: &PathBuf,
    cancel: &AtomicBool,
    report: impl Fn(Value),
) -> Result<Value, String> {
    if !command(&tool("ffmpeg"))
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "未找到FFmpeg，请将ffmpeg.exe和ffprobe.exe放在程序旁边".to_string())?
        .success()
    {
        return Err("FFmpeg无法运行".into());
    }
    let title = data["title"]
        .as_str()
        .unwrap_or("视频")
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(85)
        .collect::<String>();
    let bv = data["bv"].as_str().ok_or("缺少BV编号")?;
    let base = format!("{} [{}]", title.trim_end_matches([' ', '.']), bv);
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
    let temp = dir.join(format!(".bili-{bv}-{stamp}"));
    std::fs::create_dir(&temp).map_err(|e| e.to_string())?;
    for key in ["video", "audio"] {
        download_process::check(cancel)?;
        let url = data[key].as_str().ok_or("缺少播放地址")?;
        if !media_url_allowed(url) {
            return Err("媒体地址不是B站资源域名".into());
        }
        let file = temp.join(format!("{key}.m4s"));
        let mut child=download_process::ManagedChild::spawn(command("curl.exe").args(["--fail","--silent","--show-error","--connect-timeout","20","--max-time","1800","--retry","2","--proto","=https","-A","Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/137.0.0.0 Safari/537.36","-e",&format!("https://www.bilibili.com/video/{bv}/"),"-o"]).arg(&file).arg(url).stderr(Stdio::null()))?;
        loop {
            child.check_cancel(cancel)?;
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                if !s.success() {
                    return Err("媒体下载失败，可能链接已过期或请求受限；请重新解析后重试".into());
                }
                break;
            }
            let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            report(
                json!({"status":format!("正在下载{} · {:.1} MB",if key=="video"{"视频"}else{"音频"},size as f64/1048576.),"busy":true}),
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    report(json!({"status":"正在合并和核验…","busy":true}));
    let mut merger = download_process::ManagedChild::spawn(
        command(&tool("ffmpeg"))
            .args(["-hide_banner", "-loglevel", "error", "-n", "-i"])
            .arg(temp.join("video.m4s"))
            .arg("-i")
            .arg(temp.join("audio.m4s"))
            .args([
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-c",
                "copy",
                "-movflags",
                "+faststart",
            ])
            .arg(&out)
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )?;
    let s = merger.wait_cancellable(cancel).map_err(|error| {
        if out.exists() {
            let _ = std::fs::rename(&out, out.with_extension("incomplete.mp4"));
        }
        error
    })?;
    if !s.success() {
        return Err("音视频合并失败；临时文件保留在下载文件夹".into());
    }
    let probe = command(&tool("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,size:stream=codec_type,codec_name,width,height,avg_frame_rate",
            "-of",
            "json",
        ])
        .arg(&out)
        .output()
        .map_err(|e| e.to_string())?;
    if !probe.status.success() {
        return Err("文件已保存，但ffprobe核验失败".into());
    }
    let parsed: Value = serde_json::from_slice(&probe.stdout).map_err(|e| e.to_string())?;
    let streams = parsed["streams"].as_array().ok_or("未找到音视频流")?;
    let v = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("未找到视频")?;
    if !streams.iter().any(|s| s["codec_type"] == "audio") {
        return Err("未找到音频".into());
    }
    let duration = parsed["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or("无效时长")?;
    let expected = data["duration"].as_f64().ok_or("无效源时长")?;
    if (duration - expected).abs() > 3. {
        return Err("文件时长与源视频不一致，请重新解析；暂存文件已保留".into());
    }
    download_process::check(cancel)?;
    let size = std::fs::metadata(&out).map_err(|e| e.to_string())?.len();
    let _ = std::fs::remove_file(temp.join("video.m4s"));
    let _ = std::fs::remove_file(temp.join("audio.m4s"));
    let _ = std::fs::remove_dir(&temp);
    Ok(
        json!({"status":"下载完成 ✓","ready":true,"info":format!("{}×{} · {:.1}秒 · {:.1}MB",v["width"],v["height"],duration,size as f64/1048576.),"log":format!("保存位置：{}\n音视频核验通过，合并未重新压缩。",out.display())}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auth_urls() {
        for u in [
            "https://graph.qq.com/oauth2.0/authorize",
            "https://xui.ptlogin2.qq.com/",
            "https://passport.bilibili.com/",
            "about:blank",
            "about:blank#callback",
        ] {
            assert!(auth_navigation(u));
        }
        for u in [
            "https://qq.com.evil.org/",
            "http://graph.qq.com/",
            "https://evilqq.com/",
            "file:///C:/secret",
        ] {
            assert!(!auth_navigation(u));
        }
    }
    #[test]
    fn media_urls() {
        assert!(media_url_allowed(
            "https://upos-sz-estgcos.bilivideo.com/v.m4s"
        ));
        assert!(media_url_allowed(
            "https://xy106x41x209x206xy.mcdn.bilivideo.cn/a.m4s"
        ));
        assert!(!media_url_allowed("https://bilivideo.cn.evil.org/a"));
        assert!(!media_url_allowed("https://evilbilivideo.cn/a"));
        assert!(!media_url_allowed("http://x.bilivideo.cn/a"));
    }
    #[test]
    fn urls() {
        assert_eq!(
            bvid("https://www.bilibili.com/video/BV1234567890/?x=1").as_deref(),
            Some("BV1234567890")
        );
        assert!(bvid("https://evilbilibili.com/video/BV1234567890/").is_none());
        assert!(!host_allowed(
            "https://bilivideo.com.evil.org/a",
            "bilivideo.com"
        ));
        assert!(!host_allowed("http://x.bilivideo.com/a", "bilivideo.com"));
    }
}
