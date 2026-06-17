use crate::browser;
use anyhow::{Context, Result};
use serde_json::Value;

pub struct RedditClient {
    session: browser::Session,
}

#[derive(Debug, Clone)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    pub selftext: String,
    pub url: String,
    pub permalink: String,
    pub created_utc: f64,
    pub is_self: bool,
    pub link_flair_text: Option<String>,
    pub over_18: bool,
    pub stickied: bool,
    pub saved: bool,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub score: i64,
    pub depth: u32,
    pub created_utc: f64,
    pub replies: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub struct Subreddit {
    pub name: String,
    pub title: String,
    pub subscribers: u64,
    pub description: String,
    pub over18: bool,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub id: String,
    pub author: String,
    pub subject: String,
    pub body: String,
    pub subreddit: Option<String>,
    pub context: Option<String>,
    pub is_new: bool,
    pub item_type: String,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub name: String,
    pub link_karma: i64,
    pub comment_karma: i64,
    pub created_utc: f64,
    pub is_gold: bool,
}

impl RedditClient {
    pub fn new() -> Result<Self> {
        browser::ensure_reddit_tab()?;
        let session = browser::login_session()?;
        Ok(Self { session })
    }

    fn get(&self, endpoint: &str) -> Result<Value> {
        browser::get(endpoint)
    }

    fn post_form(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Value> {
        browser::post_form(endpoint, params, &self.session.modhash)
    }

    pub fn feed(
        &self,
        sort: &str,
        limit: u32,
        after: Option<&str>,
        subreddit: Option<&str>,
    ) -> Result<(Vec<Post>, Option<String>)> {
        let base = match subreddit {
            Some(sub) => format!("/r/{sub}/{sort}"),
            None => format!("/{sort}"),
        };
        let endpoint = match after {
            Some(a) => format!("{base}?limit={limit}&after=t3_{a}"),
            None => format!("{base}?limit={limit}"),
        };
        let resp = self.get(&endpoint)?;
        let posts = parse_posts(&resp)?;
        let next = resp["data"]["after"]
            .as_str()
            .map(|s| s.strip_prefix("t3_").unwrap_or(s).to_string());
        Ok((posts, next))
    }

    pub fn post(&self, id: &str) -> Result<Post> {
        let resp = self.get(&format!("/comments/{id}?limit=0"))?;
        let data = &resp[0]["data"]["children"][0]["data"];
        parse_post(data).context("Failed to parse post")
    }

    pub fn comments(&self, id: &str, limit: u32) -> Result<Vec<Comment>> {
        let resp = self.get(&format!("/comments/{id}?limit={limit}&depth=10"))?;

        let comments = resp[1]["data"]["children"]
            .as_array()
            .context("Invalid comments response")?;

        Ok(comments
            .iter()
            .filter(|c| c["kind"].as_str() == Some("t1"))
            .filter_map(|c| parse_comment(&c["data"]))
            .collect())
    }

    pub fn subscriptions(&self) -> Result<Vec<String>> {
        let resp = self.get("/subreddits/mine/subscriber?limit=100")?;
        let subs = resp["data"]["children"]
            .as_array()
            .context("Invalid response")?;

        let mut names: Vec<String> = subs
            .iter()
            .filter_map(|s| s["data"]["display_name"].as_str().map(|s| s.to_string()))
            .collect();
        names.sort_by_key(|a| a.to_lowercase());
        Ok(names)
    }

    pub fn subreddit_info(&self, name: &str) -> Result<Subreddit> {
        let resp = self.get(&format!("/r/{name}/about"))?;
        let data = &resp["data"];
        Ok(Subreddit {
            name: data["display_name"].as_str().unwrap_or(name).to_string(),
            title: data["title"].as_str().unwrap_or("").to_string(),
            subscribers: data["subscribers"].as_u64().unwrap_or(0),
            description: data["public_description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            over18: data["over18"].as_bool().unwrap_or(false),
        })
    }

    pub fn search(
        &self,
        query: &str,
        subreddit: Option<&str>,
        sort: &str,
        limit: u32,
    ) -> Result<Vec<Post>> {
        let base = match subreddit {
            Some(sub) => format!("/r/{sub}/search?restrict_sr=on"),
            None => "/search?".to_string(),
        };
        let endpoint = format!("{base}&q={}&sort={sort}&limit={limit}", urlencoded(query));
        let resp = self.get(&endpoint)?;
        parse_posts(&resp)
    }

    pub fn vote(&self, fullname: &str, direction: i8) -> Result<()> {
        self.post_form(
            "/api/vote",
            &[("id", fullname), ("dir", &direction.to_string())],
        )?;
        Ok(())
    }

    pub fn save(&self, fullname: &str) -> Result<()> {
        self.post_form("/api/save", &[("id", fullname)])?;
        Ok(())
    }

    pub fn unsave(&self, fullname: &str) -> Result<()> {
        self.post_form("/api/unsave", &[("id", fullname)])?;
        Ok(())
    }

    pub fn inbox(&self, limit: u32) -> Result<Vec<InboxItem>> {
        let resp = self.get(&format!("/message/inbox?limit={limit}"))?;
        let items = resp["data"]["children"]
            .as_array()
            .context("Invalid inbox response")?;

        Ok(items
            .iter()
            .filter_map(|item| parse_inbox_item(&item["data"]))
            .collect())
    }

    pub fn saved_posts(&self, limit: u32) -> Result<Vec<Post>> {
        let resp = self.get(&format!(
            "/user/{}/saved?limit={limit}",
            self.session.username
        ))?;
        let children = resp["data"]["children"]
            .as_array()
            .context("Invalid saved response")?;

        Ok(children
            .iter()
            .filter(|c| c["kind"].as_str() == Some("t3"))
            .filter_map(|c| parse_post(&c["data"]))
            .collect())
    }

    pub fn user_info(&self, username: &str) -> Result<UserInfo> {
        let resp = self.get(&format!("/user/{username}/about"))?;
        let data = &resp["data"];
        Ok(UserInfo {
            name: data["name"].as_str().unwrap_or(username).to_string(),
            link_karma: data["link_karma"].as_i64().unwrap_or(0),
            comment_karma: data["comment_karma"].as_i64().unwrap_or(0),
            created_utc: data["created_utc"].as_f64().unwrap_or(0.0),
            is_gold: data["is_gold"].as_bool().unwrap_or(false),
        })
    }

    pub fn user_posts(&self, username: &str, limit: u32) -> Result<Vec<Post>> {
        let resp = self.get(&format!(
            "/user/{username}/submitted?limit={limit}&sort=new"
        ))?;
        parse_posts(&resp)
    }

    pub fn mark_read(&self) -> Result<()> {
        self.post_form("/api/read_all_messages", &[])?;
        Ok(())
    }

    pub fn reply(&self, parent_fullname: &str, text: &str) -> Result<()> {
        self.post_form(
            "/api/comment",
            &[("thing_id", parent_fullname), ("text", text)],
        )?;
        Ok(())
    }
}

fn parse_post(data: &serde_json::Value) -> Option<Post> {
    Some(Post {
        id: data["id"].as_str()?.to_string(),
        title: html_escape::decode_html_entities(data["title"].as_str()?).to_string(),
        author: data["author"].as_str()?.to_string(),
        subreddit: data["subreddit"].as_str()?.to_string(),
        score: data["score"].as_i64()?,
        num_comments: data["num_comments"].as_i64()?,
        selftext: data["selftext"].as_str().unwrap_or("").to_string(),
        url: data["url"].as_str().unwrap_or("").to_string(),
        permalink: data["permalink"].as_str().unwrap_or("").to_string(),
        created_utc: data["created_utc"].as_f64().unwrap_or(0.0),
        is_self: data["is_self"].as_bool().unwrap_or(false),
        link_flair_text: data["link_flair_text"].as_str().map(|s| s.to_string()),
        over_18: data["over_18"].as_bool().unwrap_or(false),
        stickied: data["stickied"].as_bool().unwrap_or(false),
        saved: data["saved"].as_bool().unwrap_or(false),
    })
}

fn parse_posts(resp: &serde_json::Value) -> Result<Vec<Post>> {
    let posts = resp["data"]["children"]
        .as_array()
        .context("Invalid listing response")?
        .iter()
        .filter_map(|child| parse_post(&child["data"]))
        .collect();
    Ok(posts)
}

fn parse_comment(data: &serde_json::Value) -> Option<Comment> {
    let replies = data["replies"]["data"]["children"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|c| c["kind"].as_str() == Some("t1"))
                .filter_map(|c| parse_comment(&c["data"]))
                .collect()
        })
        .unwrap_or_default();

    Some(Comment {
        id: data["id"].as_str()?.to_string(),
        author: data["author"].as_str()?.to_string(),
        body: data["body"].as_str()?.to_string(),
        score: data["score"].as_i64()?,
        depth: data["depth"].as_u64().unwrap_or(0) as u32,
        created_utc: data["created_utc"].as_f64().unwrap_or(0.0),
        replies,
    })
}

fn parse_inbox_item(data: &serde_json::Value) -> Option<InboxItem> {
    Some(InboxItem {
        id: data["name"].as_str()?.to_string(),
        author: data["author"].as_str().unwrap_or("[deleted]").to_string(),
        subject: data["subject"].as_str().unwrap_or("").to_string(),
        body: data["body"].as_str().unwrap_or("").to_string(),
        subreddit: data["subreddit"].as_str().map(|s| s.to_string()),
        context: data["context"].as_str().map(|s| s.to_string()),
        is_new: data["new"].as_bool().unwrap_or(false),
        item_type: data["type"].as_str().unwrap_or("unknown").to_string(),
    })
}

fn urlencoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
