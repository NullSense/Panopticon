//! Unified agent session management
//!
//! This module provides a unified interface for discovering and monitoring
//! agent sessions from multiple sources (Claude Code, OpenClaw).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    UnifiedAgentWatcher                           │
//! │  ┌─────────────────┐  ┌─────────────────┐                       │
//! │  │  ClaudeWatcher  │  │ OpenClawWatcher │                       │
//! │  └────────┬────────┘  └────────┬────────┘                       │
//! │           │                    │                                │
//! │           └────────────────────┘                                │
//! │                      │                                          │
//! │              SessionMerger (pure)                               │
//! │                      │                                          │
//! │              Vec<AgentSession>                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod merger;
pub mod unified_watcher;

pub use merger::merge_sessions;
pub use unified_watcher::UnifiedAgentWatcher;
