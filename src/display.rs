use crate::client::{Comment, InboxItem, Post, Subreddit, UserInfo};

pub fn format_post_list(posts: &[Post], start_index: usize) {
    for (i, post) in posts.iter().enumerate() {
        let idx = start_index + i + 1;
        let flags = post_flags(post);
        println!(
            "{idx:3}. [{id}] r/{sub} • {score} pts • {comments} comments{flags}",
            id = post.id,
            sub = post.subreddit,
            score = post.score,
            comments = post.num_comments,
        );
        println!("     {}", post.title);
        if !post.is_self && !post.url.is_empty() {
            println!("     {}", post.url);
        }
        println!();
    }
}

pub fn format_post_detail(post: &Post) {
    let flags = post_flags(post);
    println!(
        "r/{sub} • u/{author} • {score} pts • {comments} comments{flags}",
        sub = post.subreddit,
        author = post.author,
        score = post.score,
        comments = post.num_comments,
    );
    println!();
    println!("{}", post.title);
    println!();

    if !post.selftext.is_empty() {
        println!("{}", post.selftext);
        println!();
    }

    if !post.is_self && !post.url.is_empty() {
        println!("Link: {}", post.url);
    }

    println!("https://reddit.com{}", post.permalink);
}

pub fn format_comments(comments: &[Comment]) {
    for comment in comments {
        print_comment(comment, 0);
    }
}

fn print_comment(comment: &Comment, indent: u32) {
    let pad = "  ".repeat(indent as usize);
    println!(
        "{pad}u/{author} ({score} pts)",
        author = comment.author,
        score = comment.score,
    );
    for line in comment.body.lines() {
        println!("{pad}  {line}");
    }
    println!("{pad}---");

    for reply in &comment.replies {
        print_comment(reply, indent + 1);
    }
}

pub fn format_subreddit_info(sub: &Subreddit) {
    println!("r/{}", sub.name);
    println!("  {}", sub.title);
    println!("  {} subscribers", sub.subscribers);
    if sub.over18 {
        println!("  [NSFW]");
    }
    if !sub.description.is_empty() {
        println!();
        println!("{}", sub.description);
    }
}

pub fn format_inbox(items: &[InboxItem]) {
    for item in items {
        let new_marker = if item.is_new { " [NEW]" } else { "" };
        let sub = item
            .subreddit
            .as_deref()
            .map(|s| format!(" in r/{s}"))
            .unwrap_or_default();
        println!(
            "[{type}] u/{author}{sub}{new_marker}",
            r#type = item.item_type,
            author = item.author,
        );
        println!("  {}", item.subject);
        for line in item.body.lines() {
            println!("  {line}");
        }
        println!("---");
    }
}

pub fn format_user_info(user: &UserInfo) {
    println!("u/{}", user.name);
    println!(
        "  Link karma: {}  Comment karma: {}",
        user.link_karma, user.comment_karma
    );
    if user.is_gold {
        println!("  [Gold]");
    }
    let age_days = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        - user.created_utc)
        / 86400.0;
    println!("  Account age: {:.0} days", age_days);
}

fn post_flags(post: &Post) -> String {
    let mut flags = Vec::new();
    if post.stickied {
        flags.push("pinned");
    }
    if post.over_18 {
        flags.push("nsfw");
    }
    if post.saved {
        flags.push("saved");
    }
    if let Some(ref flair) = post.link_flair_text {
        return format!(
            " [{}{}]",
            flair,
            if flags.is_empty() {
                String::new()
            } else {
                format!(", {}", flags.join(", "))
            }
        );
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    }
}
