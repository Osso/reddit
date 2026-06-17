//! Transport that routes Reddit requests through a logged-in browser tab.
//!
//! Reddit closed off free OAuth script-app creation and blocks unauthenticated
//! `.json` reads from non-browser clients, so the CLI piggybacks on an existing
//! `www.reddit.com` session: it drives Chrome via `browser-cli` to run a
//! synchronous `XMLHttpRequest` from the page context, which carries the session
//! cookies automatically. Reads hit the legacy `.json` endpoints; writes POST to
//! `/api/*` with the account modhash.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::process::Command;

const ORIGIN: &str = "https://www.reddit.com";

/// A live browser session: the logged-in user and their modhash (CSRF token for
/// write actions).
pub struct Session {
    pub username: String,
    pub modhash: String,
}

/// Ensures the active browser tab is on `www.reddit.com` so same-origin XHRs
/// carry the session cookies. Navigates there if needed.
pub fn ensure_reddit_tab() -> Result<()> {
    if current_url().starts_with(ORIGIN) {
        return Ok(());
    }
    run_browser_cli(&["open", "https://www.reddit.com/"])
        .context("Failed to open reddit.com in the browser")?;
    run_browser_cli(&["wait", "3000"]).ok();

    if !current_url().starts_with(ORIGIN) {
        bail!(
            "Could not load www.reddit.com in the browser. \
             Is Chrome running with --remote-debugging-port=9222?"
        );
    }
    Ok(())
}

/// Reads the logged-in user and modhash from `/api/me`. Fails if no one is
/// logged in (Reddit returns an empty body for anonymous sessions).
pub fn login_session() -> Result<Session> {
    let me = get("/api/me")?;
    let data = &me["data"];
    let username = data["name"]
        .as_str()
        .context("Not logged in to reddit in the browser. Run: reddit login")?
        .to_string();
    let modhash = data["modhash"].as_str().unwrap_or_default().to_string();
    Ok(Session { username, modhash })
}

/// GET a listing/info endpoint as JSON (`.json` is appended automatically).
pub fn get(endpoint: &str) -> Result<Value> {
    let url = to_json_url(endpoint);
    let js = format!(
        "(function(){{try{{\
            var x=new XMLHttpRequest();\
            x.open('GET',{url},false);\
            x.withCredentials=true;\
            x.setRequestHeader('Accept','application/json');\
            x.send();\
            if(x.status!==200)return {{__err:x.status}};\
            return JSON.parse(x.responseText);\
        }}catch(e){{return {{__err:-1,__msg:String(e)}};}}}})()",
        url = js_string(&url),
    );
    check_err(eval_json(&js)?, endpoint)
}

/// POST a form-encoded action to `/api/*`, attaching the modhash for CSRF.
pub fn post_form(endpoint: &str, params: &[(&str, &str)], modhash: &str) -> Result<Value> {
    let body = encode_form(params, modhash);
    let url = format!("{ORIGIN}{endpoint}");
    let js = format!(
        "(function(){{try{{\
            var x=new XMLHttpRequest();\
            x.open('POST',{url},false);\
            x.withCredentials=true;\
            x.setRequestHeader('Content-Type','application/x-www-form-urlencoded');\
            x.setRequestHeader('X-Modhash',{mh});\
            x.send({body});\
            if(x.status<200||x.status>=300)return {{__err:x.status}};\
            try{{return JSON.parse(x.responseText)}}catch(e){{return {{ok:true}}}}\
        }}catch(e){{return {{__err:-1,__msg:String(e)}};}}}})()",
        url = js_string(&url),
        mh = js_string(modhash),
        body = js_string(&body),
    );
    check_err(eval_json(&js)?, endpoint)
}

/// Builds the `key=value&...&uh=<modhash>&api_type=json` request body.
fn encode_form(params: &[(&str, &str)], modhash: &str) -> String {
    let mut parts: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
        .collect();
    parts.push(format!("uh={}", urlencode(modhash)));
    parts.push("api_type=json".to_string());
    parts.join("&")
}

/// Inserts `.json` before the query string for read endpoints.
fn to_json_url(endpoint: &str) -> String {
    let (path, query) = match endpoint.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (endpoint, None),
    };
    let path = if path.ends_with(".json") {
        path.to_string()
    } else {
        format!("{path}.json")
    };
    match query {
        Some(q) => format!("{ORIGIN}{path}?{q}"),
        None => format!("{ORIGIN}{path}"),
    }
}

/// Runs a JS snippet in the active tab and parses its returned value as JSON.
fn eval_json(js: &str) -> Result<Value> {
    let output = Command::new("browser-cli")
        .arg("eval")
        .arg(js)
        .output()
        .context("Failed to run browser-cli — is it installed and on PATH?")?;
    if !output.status.success() {
        bail!(
            "browser-cli eval failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("browser-cli returned non-JSON: {}", stdout.trim()))
}

/// Surfaces an `{__err: ...}` marker from the page snippet as a Rust error.
fn check_err(value: Value, context: &str) -> Result<Value> {
    if let Some(err) = value.get("__err") {
        let msg = value.get("__msg").and_then(Value::as_str).unwrap_or("");
        bail!("{context}: reddit returned {err} {msg}");
    }
    Ok(value)
}

fn current_url() -> String {
    run_browser_cli(&["get", "url"]).unwrap_or_default()
}

fn run_browser_cli(args: &[&str]) -> Result<String> {
    let output = Command::new("browser-cli")
        .args(args)
        .output()
        .context("Failed to run browser-cli — is it installed and on PATH?")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Encodes a string as a JS string literal for safe embedding in a snippet.
fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

/// Percent-encodes a value for an `application/x-www-form-urlencoded` body.
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
