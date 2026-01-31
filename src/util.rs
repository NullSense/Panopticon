//! Utility functions and helpers.

use tokio::sync::mpsc;

/// Send a value through a channel, logging a warning if it fails.
///
/// This eliminates the repetitive pattern:
/// ```ignore
/// if let Err(e) = tx.send(value).await {
///     tracing::warn!("Failed to send: {}", e);
/// }
/// ```
pub async fn send_or_log<T>(tx: &mpsc::Sender<T>, value: T, context: &str) {
    if let Err(e) = tx.send(value).await {
        tracing::warn!("Failed to send {}: {}", context, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_or_log_success() {
        let (tx, mut rx) = mpsc::channel(1);
        send_or_log(&tx, 42, "test value").await;
        assert_eq!(rx.recv().await, Some(42));
    }

    #[tokio::test]
    async fn test_send_or_log_closed_channel() {
        let (tx, rx) = mpsc::channel::<i32>(1);
        drop(rx); // Close the receiver
        // Should not panic, just log
        send_or_log(&tx, 42, "test value").await;
    }
}
