use anyhow::{Context, Result};
use crate::config::Config;

const USER_AGENT: &str = "reddit-cli/0.1 by Ossoleil";

pub async fn password_auth(
    client: &reqwest::Client,
    config: &Config,
) -> Result<(String, String)> {
    let params = [
        ("grant_type", "password"),
        ("username", config.username.as_str()),
        ("password", config.password.as_str()),
    ];

    let resp: serde_json::Value = client
        .post("https://www.reddit.com/api/v1/access_token")
        .basic_auth(&config.client_id, Some(&config.client_secret))
        .header("User-Agent", USER_AGENT)
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    let token = resp["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let error = resp["error"].as_str().unwrap_or("unknown");
            let msg = resp["message"].as_str().unwrap_or("");
            anyhow::anyhow!("Auth failed: {error} {msg}")
        })?;

    let refresh = resp["refresh_token"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_default();

    Ok((token, refresh))
}

pub async fn refresh_access_token(
    client: &reqwest::Client,
    config: &Config,
    refresh_token: &str,
) -> Result<String> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];

    let resp: serde_json::Value = client
        .post("https://www.reddit.com/api/v1/access_token")
        .basic_auth(&config.client_id, Some(&config.client_secret))
        .header("User-Agent", USER_AGENT)
        .form(&params)
        .send()
        .await
        .context("Failed to refresh token")?
        .json()
        .await?;

    resp["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Refresh token expired"))
}

pub async fn authenticate(client: &reqwest::Client, config: &Config) -> Result<String> {
    let cache = crate::config::load_token_cache();

    // Try refresh token first
    if let Some(ref refresh) = cache.refresh_token
        && let Ok(token) = refresh_access_token(client, config, refresh).await
    {
        return Ok(token);
    }

    // Fall back to password auth
    let (token, refresh_token) = password_auth(client, config).await?;

    crate::config::save_token_cache(&crate::config::TokenCache {
        refresh_token: Some(refresh_token),
    });

    Ok(token)
}
