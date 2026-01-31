//! Tests for OpenClaw file watcher
//!
//! TDD: Tests for the file-based session discovery using notify crate.

use panopticon::integrations::openclaw::watcher::OpenClawWatcher;
use std::fs;
use tempfile::TempDir;

mod watcher_creation {
    use super::*;

    #[test]
    fn creates_watcher_with_empty_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        assert!(watcher.get_sessions_snapshot().is_empty());
    }

    #[test]
    fn creates_watcher_for_nonexistent_directory() {
        // Should not panic - gracefully handles missing directory
        let result = OpenClawWatcher::new_with_path(std::path::Path::new("/nonexistent/path"));
        // May fail, but shouldn't panic
        let _ = result;
    }
}

mod session_discovery {
    use super::*;

    fn create_sessions_file(dir: &TempDir, agent_id: &str, content: &str) {
        let agent_dir = dir.path().join(agent_id).join("sessions");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("sessions.json"), content).unwrap();
    }

    #[test]
    fn discovers_single_agent_session() {
        let temp_dir = TempDir::new().unwrap();
        let sessions_json = r#"{
            "agent:default:main": {
                "sessionId": "session-123",
                "updatedAt": 9999999999999
            }
        }"#;
        create_sessions_file(&temp_dir, "default", sessions_json);

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-123");
    }

    #[test]
    fn discovers_multiple_agents() {
        let temp_dir = TempDir::new().unwrap();

        create_sessions_file(
            &temp_dir,
            "agent1",
            r#"{"k1": {"sessionId": "s1", "updatedAt": 9999999999999}}"#,
        );
        create_sessions_file(
            &temp_dir,
            "agent2",
            r#"{"k2": {"sessionId": "s2", "updatedAt": 9999999999999}}"#,
        );

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn handles_malformed_json_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        create_sessions_file(&temp_dir, "broken", "not valid json");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        // Should not panic, just return empty
        let sessions = watcher.get_sessions_snapshot();
        assert!(sessions.is_empty());
    }

    #[test]
    fn handles_empty_sessions_file() {
        let temp_dir = TempDir::new().unwrap();
        create_sessions_file(&temp_dir, "empty", "{}");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();
        assert!(sessions.is_empty());
    }
}

mod transcript_integration {
    use super::*;

    fn setup_session_with_transcript(
        dir: &TempDir,
        agent_id: &str,
        session_id: &str,
        cwd: &str,
    ) {
        let agent_dir = dir.path().join(agent_id).join("sessions");
        fs::create_dir_all(&agent_dir).unwrap();

        // Create sessions.json
        let sessions_json = format!(
            r#"{{
                "key": {{
                    "sessionId": "{}",
                    "updatedAt": 9999999999999
                }}
            }}"#,
            session_id
        );
        fs::write(agent_dir.join("sessions.json"), sessions_json).unwrap();

        // Create transcript file with ISO timestamp (real OpenClaw format)
        let transcript = format!(
            r#"{{"type":"session","id":"{}","cwd":"{}","timestamp":"2026-01-31T04:05:52.936Z"}}"#,
            session_id, cwd
        );
        fs::write(agent_dir.join(format!("{}.jsonl", session_id)), transcript).unwrap();
    }

    #[test]
    fn extracts_working_directory_from_transcript() {
        let temp_dir = TempDir::new().unwrap();
        setup_session_with_transcript(&temp_dir, "agent1", "sess-1", "/home/user/project");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].working_directory,
            Some("/home/user/project".to_string())
        );
    }
}

mod model_extraction {
    use super::*;
    use panopticon::data::{AgentStatus, AgentType};

    fn create_session_with_model(dir: &TempDir, model: &str) {
        let agent_dir = dir.path().join("agent1").join("sessions");
        fs::create_dir_all(&agent_dir).unwrap();

        let sessions_json = format!(
            r#"{{
                "key": {{
                    "sessionId": "test-session",
                    "updatedAt": 9999999999999,
                    "model": "{}"
                }}
            }}"#,
            model
        );
        fs::write(agent_dir.join("sessions.json"), sessions_json).unwrap();
    }

    #[test]
    fn extracts_opus_model() {
        let temp_dir = TempDir::new().unwrap();
        create_session_with_model(&temp_dir, "claude-opus-4-5");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].activity.model_short, Some("opus".to_string()));
    }

    #[test]
    fn extracts_sonnet_model() {
        let temp_dir = TempDir::new().unwrap();
        create_session_with_model(&temp_dir, "claude-sonnet-4");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].activity.model_short, Some("sonnet".to_string()));
    }

    #[test]
    fn status_is_running_for_recent_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        // Use a future timestamp to ensure "Running" status
        create_session_with_model(&temp_dir, "claude-opus-4-5");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentStatus::Running);
    }

    #[test]
    fn agent_type_is_openclaw() {
        let temp_dir = TempDir::new().unwrap();
        create_session_with_model(&temp_dir, "claude-opus-4-5");

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent_type, AgentType::OpenClaw);
    }
}

mod status_inference {
    use super::*;
    use panopticon::data::AgentStatus;

    fn create_session_with_timestamp(dir: &TempDir, updated_at: i64) {
        let agent_dir = dir.path().join("agent1").join("sessions");
        fs::create_dir_all(&agent_dir).unwrap();

        let sessions_json = format!(
            r#"{{
                "key": {{
                    "sessionId": "test-session",
                    "updatedAt": {}
                }}
            }}"#,
            updated_at
        );
        fs::write(agent_dir.join("sessions.json"), sessions_json).unwrap();
    }

    #[test]
    fn status_is_done_for_old_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        // Use a timestamp from 2020 (definitely more than 1 hour ago)
        create_session_with_timestamp(&temp_dir, 1577836800000); // 2020-01-01

        let watcher = OpenClawWatcher::new_with_path(temp_dir.path()).unwrap();
        let sessions = watcher.get_sessions_snapshot();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentStatus::Done);
    }
}
