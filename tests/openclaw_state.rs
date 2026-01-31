//! Tests for OpenClaw state file parsing
//!
//! TDD: These tests define the expected behavior for parsing OpenClaw's
//! sessions.json and transcript files.

use panopticon::integrations::openclaw::state::{
    OpenClawSessionsFile, SessionEntry, TranscriptHeader,
};

mod sessions_file_parsing {
    use super::*;

    #[test]
    fn parses_empty_sessions_file() {
        let json = "{}";
        let parsed: OpenClawSessionsFile = serde_json::from_str(json).unwrap();
        assert!(parsed.sessions.is_empty());
    }

    #[test]
    fn parses_single_session() {
        let json = r#"{
            "agent:default:main": {
                "sessionId": "abc123",
                "updatedAt": 1706745600000,
                "totalTokens": 4700
            }
        }"#;
        let parsed: OpenClawSessionsFile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.sessions.len(), 1);

        let (key, entry) = parsed.sessions.iter().next().unwrap();
        assert_eq!(key, "agent:default:main");
        assert_eq!(entry.session_id, "abc123");
        assert_eq!(entry.updated_at, 1706745600000);
        assert_eq!(entry.total_tokens, Some(4700));
    }

    #[test]
    fn parses_multiple_sessions() {
        let json = r#"{
            "agent:default:main": {
                "sessionId": "session1",
                "updatedAt": 1706745600000
            },
            "agent:project:feature": {
                "sessionId": "session2",
                "updatedAt": 1706745700000
            }
        }"#;
        let parsed: OpenClawSessionsFile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.sessions.len(), 2);
    }

    #[test]
    fn parses_session_with_all_optional_fields() {
        let json = r#"{
            "agent:default:main": {
                "sessionId": "abc123",
                "updatedAt": 1706745600000,
                "chatType": "direct",
                "inputTokens": 1500,
                "outputTokens": 3200,
                "totalTokens": 4700,
                "modelOverride": "anthropic/claude-opus-4-5"
            }
        }"#;
        let parsed: OpenClawSessionsFile = serde_json::from_str(json).unwrap();
        let entry = parsed.sessions.get("agent:default:main").unwrap();

        assert_eq!(entry.session_id, "abc123");
        assert_eq!(entry.chat_type, Some("direct".to_string()));
        assert_eq!(entry.input_tokens, Some(1500));
        assert_eq!(entry.output_tokens, Some(3200));
        assert_eq!(entry.total_tokens, Some(4700));
        assert_eq!(
            entry.model_override,
            Some("anthropic/claude-opus-4-5".to_string())
        );
    }

    #[test]
    fn parses_session_with_missing_optional_fields() {
        let json = r#"{
            "agent:default:main": {
                "sessionId": "minimal",
                "updatedAt": 1706745600000
            }
        }"#;
        let parsed: OpenClawSessionsFile = serde_json::from_str(json).unwrap();
        let entry = parsed.sessions.get("agent:default:main").unwrap();

        assert_eq!(entry.session_id, "minimal");
        assert_eq!(entry.chat_type, None);
        assert_eq!(entry.input_tokens, None);
        assert_eq!(entry.output_tokens, None);
        assert_eq!(entry.total_tokens, None);
        assert_eq!(entry.model_override, None);
    }
}

mod transcript_header_parsing {
    use super::*;

    #[test]
    fn parses_transcript_header_with_int_timestamp() {
        let jsonl = r#"{"type":"session","id":"abc123","cwd":"/home/user/project","timestamp":1706745600}"#;
        let header: TranscriptHeader = serde_json::from_str(jsonl).unwrap();

        assert_eq!(header.entry_type, "session");
        assert_eq!(header.id, "abc123");
        assert_eq!(header.cwd, "/home/user/project");
    }

    #[test]
    fn parses_transcript_header_with_string_timestamp() {
        // Real OpenClaw format uses ISO string timestamps
        let jsonl = r#"{"type":"session","id":"abc123","cwd":"/home/user/project","timestamp":"2026-01-31T04:05:52.936Z"}"#;
        let header: TranscriptHeader = serde_json::from_str(jsonl).unwrap();

        assert_eq!(header.entry_type, "session");
        assert_eq!(header.id, "abc123");
        assert_eq!(header.cwd, "/home/user/project");
    }

    #[test]
    fn parses_transcript_header_without_timestamp() {
        let jsonl = r#"{"type":"session","id":"abc123","cwd":"/home/user/project"}"#;
        let header: TranscriptHeader = serde_json::from_str(jsonl).unwrap();

        assert_eq!(header.cwd, "/home/user/project");
    }
}

mod session_entry {
    use super::*;

    #[test]
    fn session_entry_has_correct_defaults() {
        let json = r#"{"sessionId": "test", "updatedAt": 0}"#;
        let entry: SessionEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.session_id, "test");
        assert_eq!(entry.updated_at, 0);
        assert_eq!(entry.total_tokens, None);
    }
}
