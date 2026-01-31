//! Tests for Vercel URL encoding
//!
//! Verifies that branch names with special characters are properly URL encoded.

#[test]
fn test_branch_with_slash_encodes_correctly() {
    // Branches like "fix/issue-123" should encode the slash
    let branch = "fix/issue-123";
    let encoded = urlencoding::encode(branch);
    assert_eq!(encoded, "fix%2Fissue-123");
}

#[test]
fn test_branch_with_space_encodes_correctly() {
    let branch = "my branch";
    let encoded = urlencoding::encode(branch);
    assert_eq!(encoded, "my%20branch");
}

#[test]
fn test_branch_with_multiple_special_chars() {
    let branch = "feature/add user/auth";
    let encoded = urlencoding::encode(branch);
    assert_eq!(encoded, "feature%2Fadd%20user%2Fauth");
}

#[test]
fn test_simple_branch_unchanged() {
    let branch = "main";
    let encoded = urlencoding::encode(branch);
    assert_eq!(encoded, "main");
}

#[test]
fn test_branch_with_hyphen_and_underscore_unchanged() {
    // Hyphens and underscores are safe in URLs
    let branch = "feature-add_something";
    let encoded = urlencoding::encode(branch);
    assert_eq!(encoded, "feature-add_something");
}

#[test]
fn test_url_construction_with_encoded_branch() {
    let branch = "fix/issue-123";
    let encoded = urlencoding::encode(branch);
    let url = format!(
        "https://api.vercel.com/v6/deployments?limit=1&meta-githubCommitRef={}",
        encoded
    );
    assert_eq!(
        url,
        "https://api.vercel.com/v6/deployments?limit=1&meta-githubCommitRef=fix%2Fissue-123"
    );
}
