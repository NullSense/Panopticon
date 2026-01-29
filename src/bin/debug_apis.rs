use anyhow::Result;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config_path = dirs::config_dir()
        .map(|d| d.join("panopticon").join("config.toml"))
        .expect("Could not find config dir");

    println!("Loading config from: {}", config_path.display());

    let content = std::fs::read_to_string(&config_path)?;
    let config: toml::Value = toml::from_str(&content)?;

    let linear_token = config["tokens"]["linear"].as_str().unwrap_or("");
    let github_token = config["tokens"]["github"].as_str().unwrap_or("");
    let vercel_token = config["tokens"]["vercel"].as_str();

    println!("\n=== Testing Linear API ===");
    test_linear(linear_token).await;

    println!("\n=== Testing GitHub API ===");
    test_github(github_token).await;

    println!("\n=== Testing Vercel API ===");
    if let Some(token) = vercel_token {
        test_vercel(token).await;
    } else {
        println!("No Vercel token configured, skipping");
    }

    Ok(())
}

async fn test_linear(token: &str) {
    let client = reqwest::Client::new();

    let query = r#"{ "query": "{ viewer { id name email } }" }"#;

    let response = client
        .post("https://api.linear.app/graphql")
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .body(query)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("Status: {}", status);
            if status.is_success() {
                println!("Response: {}", &body[..body.len().min(500)]);
            } else {
                println!("Error: {}", body);
            }
        }
        Err(e) => println!("Request failed: {}", e),
    }
}

async fn test_github(token: &str) {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("Status: {}", status);
            if status.is_success() {
                let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                println!("Logged in as: {}", parsed["login"]);
            } else {
                println!("Error: {}", body);
            }
        }
        Err(e) => println!("Request failed: {}", e),
    }
}

async fn test_vercel(token: &str) {
    let client = reqwest::Client::new();

    // First, test basic auth by getting user info
    println!("\n--- Testing /v2/user ---");
    let response = client
        .get("https://api.vercel.com/v2/user")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("Status: {}", status);
            if status.is_success() {
                let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                println!("User: {}", parsed["user"]["username"]);
            } else {
                println!("Error body: {}", body);
            }
        }
        Err(e) => println!("Request failed: {}", e),
    }

    // Test listing projects
    println!("\n--- Testing /v9/projects ---");
    let response = client
        .get("https://api.vercel.com/v9/projects")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("Status: {}", status);
            if status.is_success() {
                let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                if let Some(projects) = parsed["projects"].as_array() {
                    println!("Found {} projects:", projects.len());
                    for p in projects.iter().take(5) {
                        println!("  - {}", p["name"]);
                    }
                }
            } else {
                println!("Error body: {}", body);
            }
        }
        Err(e) => println!("Request failed: {}", e),
    }

    // Test listing deployments
    println!("\n--- Testing /v6/deployments ---");
    let response = client
        .get("https://api.vercel.com/v6/deployments?limit=5")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("Status: {}", status);
            if status.is_success() {
                let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                if let Some(deployments) = parsed["deployments"].as_array() {
                    println!("Found {} recent deployments:", deployments.len());
                    for d in deployments.iter().take(5) {
                        println!("  - {} ({})", d["url"], d["readyState"]);
                    }
                }
            } else {
                println!("Error body: {}", body);
            }
        }
        Err(e) => println!("Request failed: {}", e),
    }
}
