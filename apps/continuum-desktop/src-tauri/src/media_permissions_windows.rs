//! WebView2 microphone permission is local-app-only and granted per request.
//! Screen selection remains the Windows sharing dialog; camera is never allowed.
use tauri::Manager;
use webview2_com::{
    take_pwstr, Microsoft::Web::WebView2::Win32::*, PermissionRequestedEventHandler,
};
use windows::core::{Interface, PWSTR};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_YESNO,
};

fn trusted_uri(uri: &str) -> bool {
    let Ok(url) = tauri::Url::parse(uri) else {
        return false;
    };
    ((url.scheme() == "http" || url.scheme() == "https")
        && url.host_str() == Some("tauri.localhost")
        && url.port().is_none())
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port() == Some(1420))
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window.with_webview(move |platform| {
        let install = || -> windows::core::Result<()> {
            // During setup the runtime's raw window handle may not be published
            // yet. The WebView callback runs with an initialized controller.
            let controller = platform.controller();
            let mut parent = Default::default();
            unsafe { controller.ParentWindow(&mut parent)?; }
            let parent = parent.0 as isize;
            let view = unsafe { controller.CoreWebView2()? };
            let mut token = 0;
            unsafe {
                view.add_PermissionRequested(&PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()); };
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    if kind != COREWEBVIEW2_PERMISSION_KIND_MICROPHONE && kind != COREWEBVIEW2_PERMISSION_KIND_CAMERA { return Ok(()); }
                    let mut raw = PWSTR::null();
                    args.Uri(&mut raw)?;
                    let uri = take_pwstr(raw);
                    if let Ok(session) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() { session.SetSavesInProfile(false)?; }
                    let allowed = if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE && trusted_uri(&uri) {
                        let message: Vec<u16> = "Allow microphone access for this recording? You can stop recording at any time.".encode_utf16().chain(Some(0)).collect();
                        let title: Vec<u16> = "Continuum · Capture permission".encode_utf16().chain(Some(0)).collect();
                        MessageBoxW(parent as _, message.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES
                    } else { false };
                    args.SetState(if allowed { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DENY })?;
                    Ok(())
                })), &mut token)?;
            }
            Ok(())
        };
        if let Err(error) = install() { eprintln!("Could not register microphone permission handler: {error}"); }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn microphone_permission_rejects_remote_and_lookalike_origins() {
        assert!(trusted_uri("http://tauri.localhost/index.html"));
        assert!(trusted_uri("https://tauri.localhost/index.html"));
        for origin in [
            "https://evil.example",
            "http://tauri.localhost.evil.example",
            "http://tauri.localhost:8000",
            "file:///C:/page.html",
        ] {
            assert!(!trusted_uri(origin));
        }
    }
}
