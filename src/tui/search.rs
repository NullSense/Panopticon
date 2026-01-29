//! Advanced fuzzy search with multi-term support using nucleo
//!
//! Provides fuse.js-like search capabilities:
//! - Multi-term search (whitespace splits terms, ALL must match)
//! - Weighted field scoring
//! - Smart case handling

use crate::data::Workstream;
use nucleo::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

/// Result of a search match with score and context
pub struct SearchResult {
    pub score: u32,
    pub matched_field: String,
    pub excerpt: String,
}

/// Fuzzy searcher with multi-term support
pub struct FuzzySearch {
    matcher: Matcher,
}

impl Default for FuzzySearch {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzySearch {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Match a single term against text, return score if matched
    fn match_term(&mut self, term: &str, haystack: &str) -> Option<u32> {
        if term.is_empty() || haystack.is_empty() {
            return if term.is_empty() { Some(0) } else { None };
        }

        let pattern = Pattern::parse(term, CaseMatching::Ignore, Normalization::Smart);
        let mut haystack_buf = Vec::new();
        let haystack_utf32 = Utf32Str::new(haystack, &mut haystack_buf);

        pattern.score(haystack_utf32, &mut self.matcher)
    }

    /// Multi-term search: split query on whitespace, ALL terms must match (AND semantics)
    /// Returns total score if all terms match, None otherwise
    pub fn multi_term_match(&mut self, query: &str, haystack: &str) -> Option<u32> {
        let terms: Vec<&str> = query.split_whitespace().collect();

        if terms.is_empty() {
            return Some(0);
        }

        let mut total_score = 0u32;

        for term in terms {
            match self.match_term(term, haystack) {
                Some(score) => total_score = total_score.saturating_add(score),
                None => return None, // Any term not matching = no match
            }
        }

        Some(total_score)
    }

    /// Search a workstream across all relevant fields
    /// Returns (score, matched_field, excerpt) if matched
    pub fn search_workstream(&mut self, ws: &Workstream, query: &str) -> Option<SearchResult> {
        let issue = &ws.linear_issue;

        // Define fields to search with their weights (higher = more important)
        // Weight multipliers affect the final score
        let fields: Vec<(&str, u32, &str)> = vec![
            (&issue.identifier, 1000, "identifier"),
            (&issue.title, 800, "title"),
        ];

        let mut best_result: Option<SearchResult> = None;

        // Check direct fields
        for (text, weight, field_name) in &fields {
            if let Some(score) = self.multi_term_match(query, text) {
                let weighted_score = score.saturating_mul(*weight / 100);
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: field_name.to_string(),
                        excerpt: text.to_string(),
                    });
                }
            }
        }

        // Check optional description
        if let Some(desc) = &issue.description {
            if let Some(score) = self.multi_term_match(query, desc) {
                let weighted_score = score.saturating_mul(4); // weight 400
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "description".to_string(),
                        excerpt: create_excerpt(desc, query, 80),
                    });
                }
            }
        }

        // Check team
        if let Some(team) = &issue.team {
            if let Some(score) = self.multi_term_match(query, team) {
                let weighted_score = score.saturating_mul(3); // weight 300
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "team".to_string(),
                        excerpt: format!("Team: {}", team),
                    });
                }
            }
        }

        // Check project
        if let Some(project) = &issue.project {
            if let Some(score) = self.multi_term_match(query, project) {
                let weighted_score = score.saturating_mul(3); // weight 300
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "project".to_string(),
                        excerpt: format!("Project: {}", project),
                    });
                }
            }
        }

        // Check cycle
        if let Some(cycle) = &issue.cycle {
            if let Some(score) = self.multi_term_match(query, &cycle.name) {
                let weighted_score = score.saturating_mul(3); // weight 300
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "cycle".to_string(),
                        excerpt: format!("Cycle: {}", cycle.name),
                    });
                }
            }
        }

        // Check labels
        for label in &issue.labels {
            if let Some(score) = self.multi_term_match(query, &label.name) {
                let weighted_score = score.saturating_mul(2); // weight 200
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "label".to_string(),
                        excerpt: format!("Label: {}", label.name),
                    });
                }
            }
        }

        // Check parent issue
        if let Some(parent) = &issue.parent {
            let parent_text = format!("{} {}", parent.identifier, parent.title);
            if let Some(score) = self.multi_term_match(query, &parent_text) {
                let weighted_score = score.saturating_mul(2); // weight 200
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "parent".to_string(),
                        excerpt: format!("Parent: {} {}", parent.identifier, parent.title),
                    });
                }
            }
        }

        // Check children
        for child in &issue.children {
            let child_text = format!("{} {}", child.identifier, child.title);
            if let Some(score) = self.multi_term_match(query, &child_text) {
                let weighted_score = score.saturating_mul(2); // weight 200
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "child".to_string(),
                        excerpt: format!("Sub-issue: {} {}", child.identifier, child.title),
                    });
                }
            }
        }

        // Check attachments
        for attachment in &issue.attachments {
            if let Some(score) = self.multi_term_match(query, &attachment.title) {
                let weighted_score = score.saturating_mul(1); // weight 100
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "attachment".to_string(),
                        excerpt: format!("Attachment: {}", attachment.title),
                    });
                }
            }
        }

        // Check GitHub PR
        if let Some(pr) = &ws.github_pr {
            let pr_text = format!("PR#{} {}", pr.number, pr.branch);
            if let Some(score) = self.multi_term_match(query, &pr_text) {
                let weighted_score = score.saturating_mul(4); // weight 400
                if best_result.as_ref().map_or(true, |r| weighted_score > r.score) {
                    best_result = Some(SearchResult {
                        score: weighted_score,
                        matched_field: "pr".to_string(),
                        excerpt: format!("PR #{}: {}", pr.number, pr.branch),
                    });
                }
            }
        }

        best_result
    }
}

/// Create an excerpt around the query match
fn create_excerpt(text: &str, query: &str, max_len: usize) -> String {
    // Find first term that matches
    let first_term = query.split_whitespace().next().unwrap_or(query);
    let text_lower = text.to_lowercase();
    let term_lower = first_term.to_lowercase();

    if let Some(pos) = text_lower.find(&term_lower) {
        let context_before = 20;
        let start = pos.saturating_sub(context_before);

        let excerpt: String = text.chars().skip(start).take(max_len).collect();

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if start + max_len < text.len() {
            "..."
        } else {
            ""
        };

        format!("{}{}{}", prefix, excerpt.trim(), suffix)
    } else {
        // Fallback: just take the beginning
        let excerpt: String = text.chars().take(max_len).collect();
        if text.len() > max_len {
            format!("{}...", excerpt.trim())
        } else {
            excerpt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
