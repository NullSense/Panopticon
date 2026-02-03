//! Tests for AgentType rename: Clawdbot → OpenClaw
//!
//! TDD: These tests verify the rename is complete and correct.

use panopticon::data::AgentType;

#[test]
fn test_agent_type_openclaw_exists() {
    // After rename, OpenClaw variant must exist
    let agent = AgentType::OpenClaw;
    assert_eq!(agent.label(), "OpenClaw");
}

#[test]
fn test_agent_type_claude_code_unchanged() {
    // ClaudeCode should remain unchanged
    let agent = AgentType::ClaudeCode;
    assert_eq!(agent.label(), "Claude");
}

#[test]
fn test_agent_type_variants_are_exhaustive() {
    // Ensure we handle all variants (compiler will catch missing arms)
    fn type_label(t: AgentType) -> &'static str {
        match t {
            AgentType::ClaudeCode => "Claude",
            AgentType::OpenClaw => "OpenClaw",
        }
    }

    assert_eq!(type_label(AgentType::ClaudeCode), "Claude");
    assert_eq!(type_label(AgentType::OpenClaw), "OpenClaw");
}
