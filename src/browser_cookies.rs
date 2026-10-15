//! WebView2 requests stay on the UI thread; waiting and continuations do not.
//! Wry's synchronous cookie getter pumps a nested Windows message loop, which
//! can re-enter the window event handler while moving/resizing a window.
use std::{sync::mpsc, time::Duration};
use webview2_com::{
    take_pwstr, GetCookiesCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Cookie, ICoreWebView2CookieList, ICoreWebView2_2,
    },
};
use windows_core::{Interface, BOOL, HSTRING, PWSTR};
use wry::{WebView, WebViewExtWindows};

// Deliberately no Debug/Serialize: cookie values must not enter diagnostics.
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
    pub expires: i64,
}

unsafe fn decode(cookie: ICoreWebView2Cookie) -> windows_core::Result<BrowserCookie> {
    let mut ptr = PWSTR::null();
    cookie.Name(&mut ptr)?;
    let name = take_pwstr(ptr);
    ptr = PWSTR::null();
    cookie.Value(&mut ptr)?;
    let value = take_pwstr(ptr);
    ptr = PWSTR::null();
    cookie.Domain(&mut ptr)?;
    let domain = take_pwstr(ptr);
    ptr = PWSTR::null();
    cookie.Path(&mut ptr)?;
    let path = take_pwstr(ptr);
    let mut http_only = BOOL::default();
    cookie.IsHttpOnly(&mut http_only)?;
    let mut secure = BOOL::default();
    cookie.IsSecure(&mut secure)?;
    let mut session = BOOL::default();
    cookie.IsSession(&mut session)?;
    let mut expires = 0.0;
    cookie.Expires(&mut expires)?;
    Ok(BrowserCookie {
        name,
        value,
        domain,
        path,
        http_only: http_only.as_bool(),
        secure: secure.as_bool(),
        expires: if session.as_bool() || expires < 0.0 {
            0
        } else {
            expires as i64
        },
    })
}

unsafe fn decode_list(
    list: Option<ICoreWebView2CookieList>,
) -> windows_core::Result<Vec<BrowserCookie>> {
    let mut output = Vec::new();
    if let Some(list) = list {
        let mut count = 0;
        list.Count(&mut count)?;
        for index in 0..count {
            output.push(decode(list.GetValueAtIndex(index)?)?);
        }
    }
    Ok(output)
}

fn receive<T>(rx: mpsc::Receiver<T>, timeout: Duration) -> Result<T, String> {
    rx.recv_timeout(timeout)
        .map_err(|_| "读取网页登录状态超时，请重新打开登录浏览器后重试。".into())
}

/// Must be called on the WebView's UI thread. `done` runs on a worker thread
/// and may send owned data to the event loop, but must not access a WebView.
pub fn read(
    view: &WebView,
    url: &str,
    done: impl FnOnce(Result<Vec<BrowserCookie>, String>) + Send + 'static,
) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        done(receive(rx, Duration::from_secs(15)).and_then(|value| value));
    });
    let completion = tx.clone();
    let result = (|| -> windows_core::Result<()> {
        let native = view.webview().cast::<ICoreWebView2_2>()?;
        let handler = GetCookiesCompletedHandler::create(Box::new(move |status, list| {
            let result = status.and_then(|_| unsafe { decode_list(list) });
            let _ = completion.send(
                result.map_err(|_| "读取网页登录状态失败，请重新打开登录浏览器后重试。".into()),
            );
            Ok(())
        }));
        unsafe {
            native
                .CookieManager()?
                .GetCookies(&HSTRING::from(url), &handler)
        }
    })();
    if result.is_err() {
        let _ = tx.send(Err("无法读取网页登录状态，请更新 WebView2 后重试。".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_callback_times_out_and_late_result_is_discarded() {
        let (tx, rx) = mpsc::channel::<u8>();
        assert!(receive(rx, Duration::from_millis(1)).is_err());
        assert!(tx.send(1).is_err());
        let (tx, rx) = mpsc::channel();
        tx.send(7).unwrap();
        assert_eq!(receive(rx, Duration::from_secs(1)).unwrap(), 7);
    }
}
