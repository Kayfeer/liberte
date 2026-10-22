//! Platform WebView capture settings. Device access still requires user consent.

#[cfg(target_os = "linux")]
pub fn configure(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use std::cell::Cell;
    use std::rc::Rc;
    use gtk::prelude::*;
    use webkit2gtk::{PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, UserMediaPermissionRequestExt, WebViewExt};

    window.with_webview(|platform| {
        let view = platform.inner();
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_webrtc(true);
            settings.set_enable_media_stream(true);
            settings.set_media_playback_requires_user_gesture(false);
        }
        let audio_granted = Rc::new(Cell::new(false));
        let video_granted = Rc::new(Cell::new(false));
        view.connect_permission_request(move |view, request| {
            let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() else { return false; };
            if !view.uri().as_deref().is_some_and(trusted_origin) {
                request.deny();
                return true;
            }
            let audio = media.is_for_audio_device();
            let video = media.is_for_video_device();
            if (!audio || audio_granted.get()) && (!video || video_granted.get()) {
                request.allow();
                return true;
            }
            let device = match (audio, video) {
                (true, true) => "le microphone et la caméra",
                (true, false) => "le microphone",
                _ => "la caméra",
            };
            let parent = view.toplevel().and_then(|widget| widget.downcast::<gtk::Window>().ok());
            let dialog = gtk::MessageDialog::new(
                parent.as_ref(), gtk::DialogFlags::MODAL, gtk::MessageType::Question,
                gtk::ButtonsType::YesNo,
                &format!("Autoriser Liberté à utiliser {device} pour les appels de cette session ?"),
            );
            let request = request.clone();
            let audio_granted = audio_granted.clone();
            let video_granted = video_granted.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Yes {
                    if audio { audio_granted.set(true); }
                    if video { video_granted.set(true); }
                    request.allow();
                } else { request.deny(); }
                dialog.close();
            });
            dialog.show_all();
            true
        });
    })
}

#[cfg(not(target_os = "linux"))]
pub fn configure(_window: &tauri::WebviewWindow) -> tauri::Result<()> { Ok(()) }

#[cfg(target_os = "linux")]
fn trusted_origin(uri: &str) -> bool {
    let Ok(url) = tauri::Url::parse(uri) else { return false; };
    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port_or_known_default() == Some(5173))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn capture_permission_never_trusts_remote_websites() {
        assert!(trusted_origin("tauri://localhost/index.html"));
        assert!(!trusted_origin("https://localhost.example.com"));
        assert!(!trusted_origin("https://example.com"));
        assert!(!trusted_origin("http://127.0.0.1:9000"));
        assert!(!trusted_origin("tauri://example.com"));
    }
}
