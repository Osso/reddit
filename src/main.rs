mod browser;
mod client;
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
    /// Open reddit's login page in the browser (auth is via the browser session)
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login => {
            browser::ensure_reddit_tab()?;
            std::process::Command::new("browser-cli")
                .args(["open", "https://www.reddit.com/login/"])
                .status()
                .ok();
            eprintln!(
                "Opened reddit's login page in the browser. Log in, then re-run your command."
            );
        }
        cmd => {
            let reddit = client::RedditClient::new()?;
            run_command(&reddit, cmd)?;
        }
    }

    Ok(())
}

fn run_command(reddit: &client::RedditClient, cmd: Commands) -> Result<()> {
    match cmd {
        cmd @ (Commands::Feed { .. }
        | Commands::Post { .. }
        | Commands::Comments { .. }
        | Commands::Subs
        | Commands::Sub { .. }
        | Commands::Search { .. }) => run_read_command(reddit, cmd)?,
        cmd @ (Commands::Vote { .. }
        | Commands::Save { .. }
        | Commands::Unsave { .. }
        | Commands::Saved { .. }
        | Commands::Inbox { .. }
        | Commands::ReadAll
        | Commands::User { .. }
        | Commands::Reply { .. }) => run_action_command(reddit, cmd)?,
        Commands::Login => unreachable!(),
    }
    Ok(())
}

fn run_read_command(reddit: &client::RedditClient, cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Feed {
            sort,
            limit,
            subreddit,
            after,
        } => show_feed(reddit, sort, limit, subreddit, after)?,
        Commands::Post { id } => fetch_post(reddit, &id)?,
        Commands::Comments { id, limit } => fetch_comments(reddit, &id, limit)?,
        Commands::Subs => show_subscriptions(reddit)?,
        Commands::Sub { name } => show_subreddit(reddit, &name)?,
        Commands::Search {
            query,
            subreddit,
            sort,
            limit,
        } => search_posts(reddit, &query, subreddit, sort, limit)?,
        _ => unreachable!(),
    }
    Ok(())
}

fn run_action_command(reddit: &client::RedditClient, cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Vote { id, direction } => vote_on_thing(reddit, id, direction)?,
        Commands::Save { id } => save_post(reddit, &id)?,
        Commands::Unsave { id } => unsave_post(reddit, &id)?,
        Commands::Saved { limit } => show_saved(reddit, limit)?,
        Commands::Inbox { limit } => show_inbox(reddit, limit)?,
        Commands::ReadAll => mark_all_read(reddit)?,
        Commands::User { name, limit } => show_user(reddit, &name, limit)?,
        Commands::Reply { id, text } => post_reply(reddit, &id, &text)?,
        _ => unreachable!(),
    }
    Ok(())
}

fn fetch_post(reddit: &client::RedditClient, id: &str) -> Result<()> {
    display::format_post_detail(&reddit.post(id)?);
    Ok(())
}

fn fetch_comments(reddit: &client::RedditClient, id: &str, limit: u32) -> Result<()> {
    display::format_comments(&reddit.comments(id, limit)?);
    Ok(())
}

fn show_subreddit(reddit: &client::RedditClient, name: &str) -> Result<()> {
    display::format_subreddit_info(&reddit.subreddit_info(name)?);
    Ok(())
}

fn search_posts(
    reddit: &client::RedditClient,
    query: &str,
    subreddit: Option<String>,
    sort: SearchSort,
    limit: u32,
) -> Result<()> {
    let posts = reddit.search(query, subreddit.as_deref(), sort.as_str(), limit)?;
    display::format_post_list(&posts, 0);
    Ok(())
}

fn save_post(reddit: &client::RedditClient, id: &str) -> Result<()> {
    reddit.save(&to_fullname(id, "t3"))?;
    eprintln!("Saved.");
    Ok(())
}

fn unsave_post(reddit: &client::RedditClient, id: &str) -> Result<()> {
    reddit.unsave(&to_fullname(id, "t3"))?;
    eprintln!("Unsaved.");
    Ok(())
}

fn show_saved(reddit: &client::RedditClient, limit: u32) -> Result<()> {
    display::format_post_list(&reddit.saved_posts(limit)?, 0);
    Ok(())
}

fn mark_all_read(reddit: &client::RedditClient) -> Result<()> {
    reddit.mark_read()?;
    eprintln!("All messages marked as read.");
    Ok(())
}

fn post_reply(reddit: &client::RedditClient, id: &str, text: &str) -> Result<()> {
    reddit.reply(&to_fullname(id, "t3"), text)?;
    eprintln!("Reply posted.");
    Ok(())
}

fn show_feed(
    reddit: &client::RedditClient,
    sort: SortMode,
    limit: u32,
    subreddit: Option<String>,
    after: Option<String>,
) -> Result<()> {
    let (posts, next) =
        reddit.feed(sort.as_str(), limit, after.as_deref(), subreddit.as_deref())?;
    display::format_post_list(&posts, 0);
    if let Some(cursor) = next {
        eprintln!("Next page: --after {cursor}");
    }
    Ok(())
}

fn show_subscriptions(reddit: &client::RedditClient) -> Result<()> {
    let subs = reddit.subscriptions()?;
    for name in &subs {
        println!("r/{name}");
    }
    eprintln!("\n{} subscriptions", subs.len());
    Ok(())
}

fn vote_on_thing(
    reddit: &client::RedditClient,
    id: String,
    direction: VoteDirection,
) -> Result<()> {
    let dir: i8 = match direction {
        VoteDirection::Up => 1,
        VoteDirection::Down => -1,
        VoteDirection::None => 0,
    };
    reddit.vote(&to_fullname(&id, "t3"), dir)?;
    eprintln!("Voted.");
    Ok(())
}

fn show_inbox(reddit: &client::RedditClient, limit: u32) -> Result<()> {
    let items = reddit.inbox(limit)?;
    display::format_inbox(&items);
    let new_count = items.iter().filter(|i| i.is_new).count();
    if new_count > 0 {
        eprintln!("{new_count} new notification(s)");
    }
    Ok(())
}

fn show_user(reddit: &client::RedditClient, name: &str, limit: u32) -> Result<()> {
    let info = reddit.user_info(name)?;
    display::format_user_info(&info);
    println!();
    let posts = reddit.user_posts(name, limit)?;
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
