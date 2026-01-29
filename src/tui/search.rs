//! Advanced fuzzy search with multi-term support using nucleo
//!
//! Provides fuse.js-like search capabilities:
//! - Multi-term search (whitespace splits terms, ALL must match)
//! - Weighted field scoring (Title > ID > Description > others)
//! - Match type scoring (Exact > Prefix > Fuzzy)
//! - Recency weighting (recently updated issues rank higher)

use crate::data::Workstream;
use chrono::Utc;
use nucleo::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

/// Match quality type for scoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchType {
    /// No match
    None = 0,
    /// Fuzzy match (characters scattered)
    Fuzzy = 1,
    /// Prefix match (query starts the text)
    Prefix = 2,
    /// Exact match (query equals text, case-insensitive)
    Exact = 3,
}

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

    /// Determine the match type for a term against text
    fn match_type(&self, term: &str, haystack: &str) -> MatchType {
        let term_lower = term.to_lowercase();
        let haystack_lower = haystack.to_lowercase();

        if haystack_lower == term_lower {
            MatchType::Exact
        } else if haystack_lower.starts_with(&term_lower) {
            MatchType::Prefix
        } else {
            MatchType::Fuzzy
        }
    }

    /// Match a single term against text, return (score, match_type) if matched
    fn match_term(&mut self, term: &str, haystack: &str) -> Option<(u32, MatchType)> {
        if term.is_empty() || haystack.is_empty() {
            return if term.is_empty() {
                Some((0, MatchType::Exact))
            } else {
                None
            };
        }

        let pattern = Pattern::parse(term, CaseMatching::Ignore, Normalization::Smart);
        let mut haystack_buf = Vec::new();
        let haystack_utf32 = Utf32Str::new(haystack, &mut haystack_buf);

        pattern.score(haystack_utf32, &mut self.matcher).map(|score| {
            let match_type = self.match_type(term, haystack);
            (score, match_type)
        })
    }

    /// Multi-term search: split query on whitespace, ALL terms must match (AND semantics)
    /// Returns (total_score, best_match_type) if all terms match, None otherwise
    pub fn multi_term_match(&mut self, query: &str, haystack: &str) -> Option<(u32, MatchType)> {
        let terms: Vec<&str> = query.split_whitespace().collect();

        if terms.is_empty() {
            return Some((0, MatchType::Exact));
        }

        let mut total_score = 0u32;
        let mut best_match_type = MatchType::Exact; // Start with best, degrade if needed

        for term in terms {
            match self.match_term(term, haystack) {
                Some((score, match_type)) => {
                    total_score = total_score.saturating_add(score);
                    // The overall match type is the worst match among all terms
                    if (match_type as u8) < (best_match_type as u8) {
                        best_match_type = match_type;
                    }
                }
                None => return None, // Any term not matching = no match
            }
        }

        Some((total_score, best_match_type))
    }

    /// Calculate match score with match-type bonus
    /// Match type bonus: Exact=1000, Prefix=500, Fuzzy=100
    fn calculate_score(base_score: u32, match_type: MatchType, field_weight: u32) -> u32 {
        let match_bonus = match match_type {
            MatchType::Exact => 1000,
            MatchType::Prefix => 500,
            MatchType::Fuzzy => 100,
            MatchType::None => 0,
        };

        // Combine: (match_bonus * field_weight) + base_score
        // This ensures exact title matches always rank highest
        (match_bonus * field_weight / 100).saturating_add(base_score)
    }

    /// Calculate recency bonus based on updated_at
    /// Returns 0-200 points based on how recently the issue was updated
    fn recency_bonus(ws: &Workstream) -> u32 {
        let now = Utc::now();
        let updated = ws.linear_issue.updated_at;
        let days_ago = (now - updated).num_days().max(0) as u32;

        // Decay: 200 points if updated today, -10 points per day, min 0
        200u32.saturating_sub(days_ago.saturating_mul(10))
    }

    /// Search a workstream across all relevant fields
    /// Returns (score, matched_field, excerpt) if matched
    ///
    /// Scoring priority:
    /// 1. Title matches rank highest (field weight 1000)
    /// 2. Match type: Exact > Prefix > Fuzzy
    /// 3. Recency: recently updated issues get bonus points
    pub fn search_workstream(&mut self, ws: &Workstream, query: &str) -> Option<SearchResult> {
        let issue = &ws.linear_issue;
        let recency = Self::recency_bonus(ws);

        // Field weights: Title is most important
        // Title=1000, Identifier=800, Description=400, others=200-300
        let mut best_result: Option<SearchResult> = None;

        // Helper to check if new result is better
        let is_better = |new_score: u32, current: &Option<SearchResult>| {
            current.as_ref().map_or(true, |r| new_score > r.score)
        };

        // Check title FIRST (highest priority field)
        if let Some((base_score, match_type)) = self.multi_term_match(query, &issue.title) {
            let score = Self::calculate_score(base_score, match_type, 1000).saturating_add(recency);
            if is_better(score, &best_result) {
                best_result = Some(SearchResult {
                    score,
                    matched_field: "title".to_string(),
                    excerpt: issue.title.clone(),
                });
            }
        }

        // Check identifier (second highest)
        if let Some((base_score, match_type)) = self.multi_term_match(query, &issue.identifier) {
            let score = Self::calculate_score(base_score, match_type, 800).saturating_add(recency);
            if is_better(score, &best_result) {
                best_result = Some(SearchResult {
                    score,
                    matched_field: "identifier".to_string(),
                    excerpt: issue.identifier.clone(),
                });
            }
        }

        // Check optional description
        if let Some(desc) = &issue.description {
            if let Some((base_score, match_type)) = self.multi_term_match(query, desc) {
                let score = Self::calculate_score(base_score, match_type, 400).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "description".to_string(),
                        excerpt: create_excerpt(desc, query, 80),
                    });
                }
            }
        }

        // Check team
        if let Some(team) = &issue.team {
            if let Some((base_score, match_type)) = self.multi_term_match(query, team) {
                let score = Self::calculate_score(base_score, match_type, 300).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "team".to_string(),
                        excerpt: format!("Team: {}", team),
                    });
                }
            }
        }

        // Check project
        if let Some(project) = &issue.project {
            if let Some((base_score, match_type)) = self.multi_term_match(query, project) {
                let score = Self::calculate_score(base_score, match_type, 300).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "project".to_string(),
                        excerpt: format!("Project: {}", project),
                    });
                }
            }
        }

        // Check cycle
        if let Some(cycle) = &issue.cycle {
            if let Some((base_score, match_type)) = self.multi_term_match(query, &cycle.name) {
                let score = Self::calculate_score(base_score, match_type, 300).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "cycle".to_string(),
                        excerpt: format!("Cycle: {}", cycle.name),
                    });
                }
            }
        }

        // Check labels
        for label in &issue.labels {
            if let Some((base_score, match_type)) = self.multi_term_match(query, &label.name) {
                let score = Self::calculate_score(base_score, match_type, 200).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "label".to_string(),
                        excerpt: format!("Label: {}", label.name),
                    });
                }
            }
        }

        // Check parent issue
        if let Some(parent) = &issue.parent {
            let parent_text = format!("{} {}", parent.identifier, parent.title);
            if let Some((base_score, match_type)) = self.multi_term_match(query, &parent_text) {
                let score = Self::calculate_score(base_score, match_type, 200).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "parent".to_string(),
                        excerpt: format!("Parent: {} {}", parent.identifier, parent.title),
                    });
                }
            }
        }

        // Check children
        for child in &issue.children {
            let child_text = format!("{} {}", child.identifier, child.title);
            if let Some((base_score, match_type)) = self.multi_term_match(query, &child_text) {
                let score = Self::calculate_score(base_score, match_type, 200).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "child".to_string(),
                        excerpt: format!("Sub-issue: {} {}", child.identifier, child.title),
                    });
                }
            }
        }

        // Check attachments
        for attachment in &issue.attachments {
            if let Some((base_score, match_type)) = self.multi_term_match(query, &attachment.title) {
                let score = Self::calculate_score(base_score, match_type, 100).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
                        matched_field: "attachment".to_string(),
                        excerpt: format!("Attachment: {}", attachment.title),
                    });
                }
            }
        }

        // Check GitHub PR
        if let Some(pr) = &ws.github_pr {
            let pr_text = format!("PR#{} {}", pr.number, pr.branch);
            if let Some((base_score, match_type)) = self.multi_term_match(query, &pr_text) {
                let score = Self::calculate_score(base_score, match_type, 400).saturating_add(recency);
                if is_better(score, &best_result) {
                    best_result = Some(SearchResult {
                        score,
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
