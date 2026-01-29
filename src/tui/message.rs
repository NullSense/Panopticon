//! Message enum for Elm Architecture (TEA) pattern.
//!
//! All possible user actions in the application are represented as messages.
//! This enables unidirectional data flow and testable update logic.

use crate::data::{LinearPriority, SortMode};

/// All possible user actions in the application.
///
/// Messages are dispatched from key events and processed by the `App::update()` method.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    // ─────────────────────────────────────────────────────────────────────────
    // App lifecycle
    // ─────────────────────────────────────────────────────────────────────────
    /// Quit the application
    Quit,
    /// Start a background refresh of data
    Refresh,

    // ─────────────────────────────────────────────────────────────────────────
    // Navigation
    // ─────────────────────────────────────────────────────────────────────────
    /// Move selection up by one
    MoveUp,
    /// Move selection down by one
    MoveDown,
    /// Go to the first item
    GotoTop,
    /// Go to the last item
    GotoBottom,
    /// Page up (half screen)
    PageUp,
    /// Page down (half screen)
    PageDown,

    // ─────────────────────────────────────────────────────────────────────────
    // Selection actions
    // ─────────────────────────────────────────────────────────────────────────
    /// Expand current section
    ExpandSection,
    /// Collapse current section
    CollapseSection,
    /// Open the primary link for selected item
    OpenPrimaryLink,
    /// Open the link menu modal
    OpenLinkMenu,
    /// Teleport to Claude session
    TeleportToSession,

    // ─────────────────────────────────────────────────────────────────────────
    // Search mode
    // ─────────────────────────────────────────────────────────────────────────
    /// Enter search mode (search_all: whether to search all fields)
    EnterSearch { search_all: bool },
    /// Exit search mode without confirming
    ExitSearch,
    /// Confirm search and exit search mode
    ConfirmSearch,
    /// Add a character to search query
    SearchInput(char),
    /// Remove last character from search query
    SearchBackspace,

    // ─────────────────────────────────────────────────────────────────────────
    // Modal toggles
    // ─────────────────────────────────────────────────────────────────────────
    /// Toggle help modal
    ToggleHelp,
    /// Toggle sort menu modal
    ToggleSortMenu,
    /// Toggle filter menu modal
    ToggleFilterMenu,
    /// Toggle preview panel
    TogglePreview,
    /// Toggle resize mode
    ToggleResizeMode,
    /// Close current modal (generic close)
    CloseModal,

    // ─────────────────────────────────────────────────────────────────────────
    // Help modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Switch to a specific help tab (0-indexed)
    SetHelpTab(usize),

    // ─────────────────────────────────────────────────────────────────────────
    // Sort modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Set sort mode
    SetSortMode(SortMode),

    // ─────────────────────────────────────────────────────────────────────────
    // Filter modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Toggle a cycle filter by index
    ToggleCycleFilter(usize),
    /// Toggle a priority filter
    TogglePriorityFilter(LinearPriority),
    /// Toggle showing sub-issues
    ToggleSubIssues,
    /// Clear all filters (show all)
    ClearAllFilters,
    /// Select all filters
    SelectAllFilters,

    // ─────────────────────────────────────────────────────────────────────────
    // Resize mode
    // ─────────────────────────────────────────────────────────────────────────
    /// Exit resize mode
    ExitResizeMode,
    /// Make current column narrower
    ResizeColumnNarrower,
    /// Make current column wider
    ResizeColumnWider,
    /// Move to next column in resize mode
    ResizeNextColumn,
    /// Move to previous column in resize mode
    ResizePrevColumn,

    // ─────────────────────────────────────────────────────────────────────────
    // Link menu modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Open the links popup (nested in link menu)
    OpenLinksPopup,
    /// Close the links popup
    CloseLinksPopup,
    /// Open Linear link
    OpenLinearLink,
    /// Open GitHub link
    OpenGithubLink,
    /// Open Vercel link
    OpenVercelLink,

    // ─────────────────────────────────────────────────────────────────────────
    // Link menu navigation
    // ─────────────────────────────────────────────────────────────────────────
    /// Navigate to next child issue in list
    NextChildIssue,
    /// Navigate to previous child issue in list
    PrevChildIssue,
    /// Navigate to the currently selected child issue
    NavigateToSelectedChild,
    /// Navigate to parent issue
    NavigateToParent,
    /// Open a document by index (0-indexed)
    OpenDocument(usize),
    /// Open the description modal
    OpenDescriptionModal,
    /// Navigate to child in modal (0-indexed), opening in browser if not found
    NavigateToChild(usize),

    // ─────────────────────────────────────────────────────────────────────────
    // Description modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Scroll description by delta (positive = down, negative = up)
    ScrollDescription(i32),
    /// Close description modal
    CloseDescriptionModal,

    // ─────────────────────────────────────────────────────────────────────────
    // Modal search (within link menu)
    // ─────────────────────────────────────────────────────────────────────────
    /// Enter modal search mode
    EnterModalSearch,
    /// Exit modal search mode
    ExitModalSearch,
    /// Add character to modal search query
    ModalSearchInput(char),
    /// Remove last character from modal search query
    ModalSearchBackspace,
    /// Clear modal search query
    ClearModalSearch,

    // ─────────────────────────────────────────────────────────────────────────
    // Navigation stack
    // ─────────────────────────────────────────────────────────────────────────
    /// Navigate back in the issue navigation stack
    NavigateBack,

    // ─────────────────────────────────────────────────────────────────────────
    // Spawn agent modal
    // ─────────────────────────────────────────────────────────────────────────
    /// Open spawn agent modal
    OpenSpawnAgentModal,
    /// Close spawn agent modal
    CloseSpawnAgentModal,
    /// Add character to directory input
    SpawnDirectoryInput(char),
    /// Remove last character from directory input
    SpawnDirectoryBackspace,
    /// Select previous directory in recent list
    SpawnDirSelectUp,
    /// Select next directory in recent list
    SpawnDirSelectDown,
    /// Confirm and spawn agent
    ConfirmSpawnAgent,
    /// Clear directory input
    ClearSpawnDirectoryInput,

    // ─────────────────────────────────────────────────────────────────────────
    // No-op
    // ─────────────────────────────────────────────────────────────────────────
    /// No operation (for unhandled keys or pending chords)
    None,
}
