//! Status inference from OpenClaw session timestamps
//!
//! OpenClaw determines session activity by timestamp freshness.
//! This module provides the same logic Panopticon uses to infer status.
//!
//! # Thresholds
//!
//! | Elapsed Time | Status |
//! |--------------|--------|
//! | 0-30s | Running |
//! | 31s-5m | Idle |
//! | 5m-60m | WaitingForInput |
//! | >60m | Done |

use crate::data::AgentStatus;
use chrono::{DateTime, Utc};

/// Threshold constants (in seconds)
const RUNNING_THRESHOLD_SECS: i64 = 30;
const IDLE_THRESHOLD_SECS: i64 = 300; // 5 minutes
const WAITING_THRESHOLD_SECS: i64 = 3600; // 60 minutes

/// Infer agent status from the last update timestamp.
///
/// This is a pure function that maps elapsed time to status:
/// - 0-30 seconds: Running (actively working)
/// - 31 seconds - 5 minutes: Idle (paused but recent)
/// - 5-60 minutes: WaitingForInput (probably needs attention)
/// - >60 minutes: Done (session likely finished)
///
/// # Arguments
/// * `updated_at` - When the session was last updated
/// * `now` - Current timestamp for comparison
///
/// # Example
/// ```ignore
/// let status = infer_status(session.updated_at, Utc::now());
/// ```
pub fn infer_status(updated_at: DateTime<Utc>, now: DateTime<Utc>) -> AgentStatus {
    let elapsed_secs = now.signed_duration_since(updated_at).num_seconds();

    // Handle future timestamps (treat as most recent = Running)
    if elapsed_secs < 0 {
        return AgentStatus::Running;
    }

    match elapsed_secs {
        0..=RUNNING_THRESHOLD_SECS => AgentStatus::Running,
        secs if secs <= IDLE_THRESHOLD_SECS => AgentStatus::Idle,
        secs if secs <= WAITING_THRESHOLD_SECS => AgentStatus::WaitingForInput,
        _ => AgentStatus::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn thresholds_are_correct() {
        assert_eq!(RUNNING_THRESHOLD_SECS, 30);
        assert_eq!(IDLE_THRESHOLD_SECS, 300);
        assert_eq!(WAITING_THRESHOLD_SECS, 3600);
    }

    #[test]
    fn boundary_at_30_seconds() {
        let now = Utc::now();
        // At 30 seconds: Running
        assert_eq!(
            infer_status(now - Duration::seconds(30), now),
            AgentStatus::Running
        );
        // At 31 seconds: Idle
        assert_eq!(
            infer_status(now - Duration::seconds(31), now),
            AgentStatus::Idle
        );
    }

    #[test]
    fn boundary_at_5_minutes() {
        let now = Utc::now();
        // At 300 seconds (5m): Idle
        assert_eq!(
            infer_status(now - Duration::seconds(300), now),
            AgentStatus::Idle
        );
        // At 301 seconds: WaitingForInput
        assert_eq!(
            infer_status(now - Duration::seconds(301), now),
            AgentStatus::WaitingForInput
        );
    }

    #[test]
    fn boundary_at_60_minutes() {
        let now = Utc::now();
        // At 3600 seconds (60m): WaitingForInput
        assert_eq!(
            infer_status(now - Duration::seconds(3600), now),
            AgentStatus::WaitingForInput
        );
        // At 3601 seconds: Done
        assert_eq!(
            infer_status(now - Duration::seconds(3601), now),
            AgentStatus::Done
        );
    }
}
