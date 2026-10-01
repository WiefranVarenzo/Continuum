//! WebKitGTK denies unhandled media requests. Ask the user in the owning window;
//! screen selection still belongs to the desktop portal after this app-level grant.
use gtk::{glib, prelude::*};
use tauri::Manager;
use webkit2gtk::{PermissionRequestExt, UserMediaPermissionRequestExt, WebViewExt};

fn trusted_uri(uri: &str) -> bool {
    let Ok(url) = tauri::Url::parse(uri) else { return false; };
    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (cfg!(debug_assertions) && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port() == Some(1420))
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else { return Ok(()); };
    window.with_webview(|platform| {
        let view = platform.inner();
        view.connect_permission_request(|view, request| {
            let Some(media) = request.downcast_ref::<webkit2gtk::UserMediaPermissionRequest>() else {
                return false;
            };
            if !view.uri().is_some_and(|uri| trusted_uri(uri.as_str())) {
                request.deny();
                return true;
            }
            use glib::translate::ToGlibPtr;
            // The generated crate does not expose the display-device helper as a method.
            let display = unsafe {
                webkit2gtk::ffi::webkit_user_media_permission_is_for_display_device(media.to_glib_none().0) != 0
            };
            let microphone = media.is_for_audio_device();
            if !display && (!microphone || media.is_for_video_device()) {
                request.deny(); // Continuum has no camera capture feature.
                return true;
            }
            let parent = view.toplevel().and_then(|widget| widget.downcast::<gtk::Window>().ok());
            let message = if display {
                "Allow screen sharing for this capture? Your desktop will ask which screen or window to share."
            } else {
                "Allow microphone access for this recording? You can stop recording at any time."
            };
            let dialog = gtk::MessageDialog::new(parent.as_ref(), gtk::DialogFlags::MODAL,
                gtk::MessageType::Question, gtk::ButtonsType::None, message);
            dialog.set_title("Continuum · Capture permission");
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Allow this capture", gtk::ResponseType::Accept);
            dialog.set_default_response(gtk::ResponseType::Cancel);
            let permission = request.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept { permission.allow(); } else { permission.deny(); }
                // The response handler owns the prompt lifecycle, not the media stream.
                unsafe { dialog.destroy(); }
            });
            let weak = dialog.downgrade();
            glib::timeout_add_local_once(std::time::Duration::from_secs(85), move || {
                if let Some(dialog) = weak.upgrade() { dialog.response(gtk::ResponseType::Cancel); }
            });
            dialog.show_all();
            true
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_permission_is_limited_to_the_local_app() {
        assert!(trusted_uri("tauri://localhost/index.html"));
        assert!(!trusted_uri("https://example.org"));
        assert!(!trusted_uri("tauri://localhost.evil.example"));
        assert!(!trusted_uri("http://localhost:1421"));
    }
}
