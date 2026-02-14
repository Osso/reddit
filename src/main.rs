mod auth;
#[allow(dead_code)]
mod client;
mod config;
mod display;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "reddit")]
#[command(about = "Reddit CLI client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure Reddit API credentials
    Config {
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        client_secret: String,
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
    },
    /// Re-authenticate and cache refresh token
    Login,
    /// Browse feed (home or subreddit)
    Feed {
        /// Sort mode
        #[arg(short, long, default_value = "hot")]
        sort: SortMode,
        /// Number of posts
        #[arg(short, long, default_value = "25")]
        limit: u32,
        /// Subreddit name (omit for home feed)
        #[arg(short = 'r', long)]
        subreddit: Option<String>,
        /// Pagination cursor (post ID from previous page)
        #[arg(long)]
        after: Option<String>,
    },
    /// Show a specific post
    Post {
        /// Post ID (e.g., 1abc23)
        id: String,
    },
    /// Show comments for a post
    Comments {
        /// Post ID
        id: String,
        /// Max comments to fetch
        #[arg(short, long, default_value = "25")]
        limit: u32,
    },
    /// List subscribed subreddits
    Subs,
    /// Show subreddit info
    Sub {
        /// Subreddit name
        name: String,
    },
    /// Search posts
    Search {
        /// Search query
        query: String,
        /// Limit to subreddit
        #[arg(short = 'r', long)]
        subreddit: Option<String>,
        /// Sort results
        #[arg(short, long, default_value = "relevance")]
        sort: SearchSort,
        /// Number of results
        #[arg(short, long, default_value = "25")]
        limit: u32,
    },
    /// Vote on a post or comment
    Vote {
        /// Thing ID (post or comment, e.g., 1abc23)
        id: String,
        /// Vote direction
        direction: VoteDirection,
    },
    /// Save a post
    Save {
        /// Post ID
        id: String,
    },
    /// Unsave a post
    Unsave {
        /// Post ID
        id: String,
    },
    /// View saved posts
    Saved {
        /// Number of posts
        #[arg(short, long, default_value = "25")]
        limit: u32,
    },
    /// View inbox notifications
    Inbox {
        /// Number of items
        #[arg(short, long, default_value = "25")]
        limit: u32,
    },
    /// Mark all inbox messages as read
    ReadAll,
    /// View user profile and recent posts
    User {
        /// Username
        name: String,
        /// Number of recent posts to show
        #[arg(short, long, default_value = "5")]
        limit: u32,
    },
    /// Reply to a post or comment
    Reply {
        /// Post ID (t3_xxx) or comment ID (t1_xxx)
        id: String,
        /// Reply text (markdown)
        text: String,
    },
}

#[derive(Clone, ValueEnum)]
enum SortMode {
    Hot,
    New,
    Top,
    Rising,
    Best,
}

impl SortMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::New => "new",
            Self::Top => "top",
            Self::Rising => "rising",
            Self::Best => "best",
        }
    }
}

#[derive(Clone, ValueEnum)]
enum SearchSort {
    Relevance,
    Hot,
    Top,
    New,
    Comments,
}

impl SearchSort {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Hot => "hot",
            Self::Top => "top",
            Self::New => "new",
            Self::Comments => "comments",
        }
    }
}

#[derive(Clone, ValueEnum)]
enum VoteDirection {
    Up,
    Down,
    None,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config {
            client_id,
            client_secret,
            username,
            password,
        } => {
            config::save_config(&config::Config {
                client_id,
                client_secret,
                username,
                password,
            })?;
            eprintln!("Configuration saved to {:?}", config::config_dir());
        }
        Commands::Login => {
            let cfg = config::load_config()?;
            let http = reqwest::Client::new();
            let (_, refresh) = auth::password_auth(&http, &cfg).await?;
            config::save_token_cache(&config::TokenCache {
                refresh_token: Some(refresh),
            });
            eprintln!("Login successful. Refresh token saved.");
        }
        cmd => {
            let reddit = client::RedditClient::new().await?;
            run_command(&reddit, cmd).await?;
        }
    }

    Ok(())
}

async fn run_command(reddit: &client::RedditClient, cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Feed { sort, limit, subreddit, after } => {
            show_feed(reddit, sort, limit, subreddit, after).await?;
        }
        Commands::Post { id } => display::format_post_detail(&reddit.post(&id).await?),
        Commands::Comments { id, limit } => {
            display::format_comments(&reddit.comments(&id, limit).await?);
        }
        Commands::Subs => show_subscriptions(reddit).await?,
        Commands::Sub { name } => {
            display::format_subreddit_info(&reddit.subreddit_info(&name).await?);
        }
        Commands::Search { query, subreddit, sort, limit } => {
            let posts = reddit.search(&query, subreddit.as_deref(), sort.as_str(), limit).await?;
            display::format_post_list(&posts, 0);
        }
        Commands::Vote { id, direction } => vote_on_thing(reddit, id, direction).await?,
        Commands::Save { id } => { reddit.save(&to_fullname(&id, "t3")).await?; eprintln!("Saved."); }
        Commands::Unsave { id } => { reddit.unsave(&to_fullname(&id, "t3")).await?; eprintln!("Unsaved."); }
        Commands::Saved { limit } => display::format_post_list(&reddit.saved_posts(limit).await?, 0),
        Commands::Inbox { limit } => show_inbox(reddit, limit).await?,
        Commands::ReadAll => { reddit.mark_read().await?; eprintln!("All messages marked as read."); }
        Commands::User { name, limit } => show_user(reddit, &name, limit).await?,
        Commands::Reply { id, text } => {
            reddit.reply(&to_fullname(&id, "t3"), &text).await?;
            eprintln!("Reply posted.");
        }
        Commands::Config { .. } | Commands::Login => unreachable!(),
    }
    Ok(())
}

async fn show_feed(
    reddit: &client::RedditClient,
    sort: SortMode,
    limit: u32,
    subreddit: Option<String>,
    after: Option<String>,
) -> Result<()> {
    let (posts, next) = reddit
        .feed(sort.as_str(), limit, after.as_deref(), subreddit.as_deref())
        .await?;
    display::format_post_list(&posts, 0);
    if let Some(cursor) = next {
        eprintln!("Next page: --after {cursor}");
    }
    Ok(())
}

async fn show_subscriptions(reddit: &client::RedditClient) -> Result<()> {
    let subs = reddit.subscriptions().await?;
    for name in &subs {
        println!("r/{name}");
    }
    eprintln!("\n{} subscriptions", subs.len());
    Ok(())
}

async fn vote_on_thing(
    reddit: &client::RedditClient,
    id: String,
    direction: VoteDirection,
) -> Result<()> {
    let dir: i8 = match direction {
        VoteDirection::Up => 1,
        VoteDirection::Down => -1,
        VoteDirection::None => 0,
    };
    reddit.vote(&to_fullname(&id, "t3"), dir).await?;
    eprintln!("Voted.");
    Ok(())
}

async fn show_inbox(reddit: &client::RedditClient, limit: u32) -> Result<()> {
    let items = reddit.inbox(limit).await?;
    display::format_inbox(&items);
    let new_count = items.iter().filter(|i| i.is_new).count();
    if new_count > 0 {
        eprintln!("{new_count} new notification(s)");
    }
    Ok(())
}

async fn show_user(reddit: &client::RedditClient, name: &str, limit: u32) -> Result<()> {
    let info = reddit.user_info(name).await?;
    display::format_user_info(&info);
    println!();
    let posts = reddit.user_posts(name, limit).await?;
    if !posts.is_empty() {
        println!("Recent posts:");
        display::format_post_list(&posts, 0);
    }
    Ok(())
}

/// Add type prefix (e.g. "t3_") if not already present.
fn to_fullname(id: &str, default_prefix: &str) -> String {
    if id.starts_with("t1_") || id.starts_with("t3_") {
        id.to_string()
    } else {
        format!("{default_prefix}_{id}")
    }
}
