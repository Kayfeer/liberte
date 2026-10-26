//! Runtime selection and capture consent for the isolated CEF prototype.
//!
//! Linux uses Chrome-style CEF so default microphone/camera decisions are
//! handled by Chromium's consent prompt. Synthetic devices and remote debugging
//! are compiled only into the separate QA executable.

#[cfg(all(feature = "qa-media-devices", not(debug_assertions)))]
compile_error!(
    "qa-media-devices is restricted to debug QA builds; never enable it for distribution"
);

#[cfg(all(feature = "qa-media-devices", not(target_os = "linux")))]
compile_error!("qa-media-devices requires the Linux CEF QA runner");

#[cfg(target_os = "linux")]
pub fn cef_runtime() -> Result<tauri_runtime_cef::Cef, String> {
    use tauri_runtime_cef::{Cef, SandboxPolicy};

    // Tauri's development flag follows custom-protocol, not Rust's debug flag.
    // Without this guard an ad-hoc release could still accept Chromium argv.
    if !cfg!(debug_assertions) && tauri::is_dev() {
        return Err("CEF release builds require tauri/custom-protocol".to_string());
    }
    let runtime = Cef::default().sandbox(SandboxPolicy::Required);
    #[cfg(all(feature = "qa-media-devices", debug_assertions))]
    let runtime = {
        use tauri_runtime_cef::RemoteDebugging;
        let port = qa_cdp_port()?;
        tracing::warn!(
            port,
            "QA: synthetic Chromium devices and local CDP enabled; physical devices and the production consent prompt are not validated"
        );
        runtime
            .command_line_arg("--use-fake-device-for-media-stream", None::<String>)
            .remote_debugging(RemoteDebugging::Port {
                port,
                allowed_origins: Vec::new(),
            })
    };
    Ok(runtime)
}

#[cfg(all(feature = "qa-media-devices", debug_assertions, target_os = "linux"))]
fn qa_cdp_port() -> Result<u16, String> {
    std::env::var("LIBERTE_QA_CDP_PORT")
        .map_err(|_| "LIBERTE_QA_CDP_PORT is required in the isolated QA build".to_string())?
        .parse::<u16>()
        .ok()
        .filter(|port| *port >= 1024)
        .ok_or_else(|| "LIBERTE_QA_CDP_PORT must be between 1024 and 65535".to_string())
}

/// Describes compile-time QA configuration, never claims capture is active.
#[cfg(all(feature = "qa-media-devices", debug_assertions, target_os = "linux"))]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaRuntimeConfiguration {
    engine: &'static str,
    synthetic_media: bool,
    remote_debugging_port: u16,
    debug_build: bool,
}

#[cfg(all(feature = "qa-media-devices", debug_assertions, target_os = "linux"))]
#[tauri::command]
pub fn qa_runtime_configuration() -> Result<QaRuntimeConfiguration, String> {
    Ok(QaRuntimeConfiguration {
        engine: "cef",
        synthetic_media: true,
        remote_debugging_port: qa_cdp_port()?,
        debug_build: true,
    })
}

/// Construct the window with its navigation and consent policy already attached.
/// The config's `create: false` avoids a window loading before these handlers.
pub fn create_main_window(app: &tauri::App) -> tauri::Result<tauri::WebviewWindow> {
    use tauri::webview::{NewWindowResponse, PermissionResponse};

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .expect("The main window configuration is required");
    let builder = tauri::WebviewWindowBuilder::from_config(app, config)?
        .on_navigation(|url| trusted_origin(url.as_str()))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        // In Chrome style Default retains Chromium's prompt and stored decisions.
        // Never blanket-Allow camera/microphone or use --enable-media-stream.
        .on_permission_request(|_, _| PermissionResponse::Default);
    #[cfg(target_os = "linux")]
    let builder = {
        use tauri_runtime_cef::{RuntimeStyle, WebviewWindowBuilderCefExt};
        builder.browser_runtime_style(RuntimeStyle::Chrome)
    };
    builder.build()
}

fn trusted_origin(uri: &str) -> bool {
    let Ok(url) = tauri::Url::parse(uri) else {
        return false;
    };
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let app_origin = match url.scheme() {
        "tauri" => url.host_str() == Some("localhost") && url.port().is_none(),
        "http" | "https" => url.host_str() == Some("tauri.localhost") && url.port().is_none(),
        _ => false,
    };
    app_origin
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port_or_known_default() == Some(5173))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_permission_never_trusts_remote_websites() {
        assert!(trusted_origin("tauri://localhost/index.html"));
        assert!(trusted_origin("http://tauri.localhost/index.html"));
        assert!(trusted_origin("https://tauri.localhost/index.html"));
        for uri in [
            "https://localhost.example.com",
            "https://tauri.localhost.example.com",
            "http://tauri.localhost:9000",
            "https://user@tauri.localhost",
            "https://example.com",
            "http://127.0.0.1:9000",
            "tauri://example.com",
        ] {
            assert!(!trusted_origin(uri), "untrusted navigation: {uri}");
        }
    }
}
