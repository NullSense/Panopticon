//! Tests for TUI fuzzy search functionality.
//!
//! Tests the multi-term fuzzy search matching algorithm.

use panopticon::tui::search::FuzzySearch;

// ============================================================================
// Basic Matching Tests
// ============================================================================

#[test]
fn test_single_term_match() {
    let mut search = FuzzySearch::new();
    assert!(search.multi_term_match("test", "this is a test").is_some());
    assert!(search.multi_term_match("xyz", "this is a test").is_none());
}

#[test]
fn test_multi_term_match() {
    let mut search = FuzzySearch::new();
    // All terms must match
    assert!(search
        .multi_term_match("test case", "this is a test case")
        .is_some());
    assert!(search
        .multi_term_match("test xyz", "this is a test case")
        .is_none());
}

#[test]
fn test_case_insensitive() {
    let mut search = FuzzySearch::new();
    assert!(search.multi_term_match("TEST", "this is a test").is_some());
    assert!(search.multi_term_match("Test", "THIS IS A TEST").is_some());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_query() {
    let mut search = FuzzySearch::new();
    // Empty query should match everything or nothing depending on implementation
    // Testing expected behavior
    let result = search.multi_term_match("", "some text");
    // Empty query typically matches anything
    assert!(result.is_some());
}

#[test]
fn test_empty_haystack() {
    let mut search = FuzzySearch::new();
    assert!(search.multi_term_match("test", "").is_none());
}

#[test]
fn test_whitespace_only_query() {
    let mut search = FuzzySearch::new();
    // Whitespace-only query should behave like empty query
    let result = search.multi_term_match("   ", "some text");
    assert!(result.is_some());
}

#[test]
fn test_partial_word_match() {
    let mut search = FuzzySearch::new();
    // Fuzzy search should match partial words
    assert!(search.multi_term_match("tes", "testing").is_some());
}

#[test]
fn test_multiple_spaces_between_terms() {
    let mut search = FuzzySearch::new();
    // Multiple spaces should still work as term separators
    assert!(search
        .multi_term_match("test   case", "this is a test case")
        .is_some());
}
