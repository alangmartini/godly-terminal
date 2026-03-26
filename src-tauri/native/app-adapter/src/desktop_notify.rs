/// Send an OS-level desktop notification on a background thread.
///
/// Fire-and-forget: the notification is dispatched asynchronously so the
/// Iced event loop is never blocked.
pub fn send_desktop_notification(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    let _ = std::thread::Builder::new()
        .name("desktop-notify".to_string())
        .spawn(move || {
            if let Err(e) = notify_rust::Notification::new()
                .appname("Godly Terminal")
                .summary(&title)
                .body(&body)
                .show()
            {
                log::warn!("Desktop notification failed: {e}");
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_does_not_panic() {
        // Just verify the function doesn't panic when spawning the thread.
        // The actual OS notification may or may not display in CI.
        send_desktop_notification("Test", "Hello from tests");
    }
}
