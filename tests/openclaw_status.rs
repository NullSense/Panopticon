//! Tests for OpenClaw status inference from timestamps
//!
//! TDD: These tests define the expected behavior for inferring AgentStatus
//! from session timestamps. Pure function with 100% coverage.

use chrono::{Duration, Utc};
use panopticon::data::AgentStatus;
use panopticon::integrations::openclaw::status::infer_status;

mod running_status {
    use super::*;

    #[test]
    fn returns_running_when_updated_just_now() {
        let now = Utc::now();
        let updated = now;
        assert_eq!(infer_status(updated, now), AgentStatus::Running);
    }

    #[test]
    fn returns_running_when_updated_10_seconds_ago() {
        let now = Utc::now();
        let updated = now - Duration::seconds(10);
        assert_eq!(infer_status(updated, now), AgentStatus::Running);
    }

    #[test]
    fn returns_running_when_updated_29_seconds_ago() {
        let now = Utc::now();
        let updated = now - Duration::seconds(29);
        assert_eq!(infer_status(updated, now), AgentStatus::Running);
    }

    #[test]
    fn returns_running_at_exactly_30_seconds() {
        let now = Utc::now();
        let updated = now - Duration::seconds(30);
        assert_eq!(infer_status(updated, now), AgentStatus::Running);
    }
}

mod idle_status {
    use super::*;

    #[test]
    fn returns_idle_when_updated_31_seconds_ago() {
        let now = Utc::now();
        let updated = now - Duration::seconds(31);
        assert_eq!(infer_status(updated, now), AgentStatus::Idle);
    }

    #[test]
    fn returns_idle_when_updated_2_minutes_ago() {
        let now = Utc::now();
        let updated = now - Duration::minutes(2);
        assert_eq!(infer_status(updated, now), AgentStatus::Idle);
    }

    #[test]
    fn returns_idle_at_exactly_5_minutes() {
        let now = Utc::now();
        let updated = now - Duration::minutes(5);
        assert_eq!(infer_status(updated, now), AgentStatus::Idle);
    }
}

mod waiting_status {
    use super::*;

    #[test]
    fn returns_waiting_when_updated_5_minutes_1_second_ago() {
        let now = Utc::now();
        let updated = now - Duration::minutes(5) - Duration::seconds(1);
        assert_eq!(infer_status(updated, now), AgentStatus::WaitingForInput);
    }

    #[test]
    fn returns_waiting_when_updated_30_minutes_ago() {
        let now = Utc::now();
        let updated = now - Duration::minutes(30);
        assert_eq!(infer_status(updated, now), AgentStatus::WaitingForInput);
    }

    #[test]
    fn returns_waiting_at_exactly_60_minutes() {
        let now = Utc::now();
        let updated = now - Duration::minutes(60);
        assert_eq!(infer_status(updated, now), AgentStatus::WaitingForInput);
    }
}

mod done_status {
    use super::*;

    #[test]
    fn returns_done_when_updated_61_minutes_ago() {
        let now = Utc::now();
        let updated = now - Duration::minutes(61);
        assert_eq!(infer_status(updated, now), AgentStatus::Done);
    }

    #[test]
    fn returns_done_when_updated_2_hours_ago() {
        let now = Utc::now();
        let updated = now - Duration::hours(2);
        assert_eq!(infer_status(updated, now), AgentStatus::Done);
    }

    #[test]
    fn returns_done_when_updated_24_hours_ago() {
        let now = Utc::now();
        let updated = now - Duration::hours(24);
        assert_eq!(infer_status(updated, now), AgentStatus::Done);
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn handles_future_timestamp_gracefully() {
        let now = Utc::now();
        let future = now + Duration::seconds(10);
        // Future timestamps should be treated as Running (most recent)
        assert_eq!(infer_status(future, now), AgentStatus::Running);
    }
}
