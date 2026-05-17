use crate::api::{Api, ApiConfig};
use crate::error::Result;
use crate::login::LoginConfig;
use crate::pool::AccountsPool;
use crate::utils::{parse_cookies, print_table};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

#[derive(Debug, Parser)]
#[command(name = "xscraper")]
#[command(version, about = "Rust X/Twitter GraphQL scraper")]
pub struct Cli {
    #[arg(long, global = true, default_value = "accounts.db", env = "XSCRAPER_DB")]
    pub db: PathBuf,

    #[arg(long, global = true, env = "XSCRAPER_PROXY")]
    pub proxy: Option<String>,

    #[arg(long, global = true, env = "XSCRAPER_BASE_URL", default_value = "https://x.com")]
    pub base_url: String,

    #[arg(long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Version,
    Accounts,
    Stats,
    #[command(alias = "add_accounts")]
    AddAccounts(AddAccountsArgs),
    AddCookie(AddCookieArgs),
    #[command(alias = "del_accounts")]
    DelAccounts(UsersArgs),
    #[command(alias = "delete_inactive")]
    DeleteInactive,
    #[command(alias = "login_accounts")]
    LoginAccounts(LoginArgs),
    Relogin(ReloginArgs),
    #[command(alias = "relogin_failed")]
    ReloginFailed(LoginArgs),
    #[command(alias = "reset_locks")]
    ResetLocks,
    Search(SearchArgs),
    #[command(alias = "search_trend")]
    SearchTrend(SearchArgs),
    #[command(alias = "search_user")]
    SearchUser(SearchArgs),
    #[command(alias = "tweet_details")]
    TweetDetails(TweetArgs),
    #[command(alias = "tweet_replies")]
    TweetReplies(LimitedTweetArgs),
    Retweeters(LimitedTweetArgs),
    #[command(alias = "user_by_id")]
    UserById(UserIdArgs),
    #[command(alias = "user_by_login")]
    UserByLogin(UsernameArgs),
    Following(LimitedUserArgs),
    Followers(LimitedUserArgs),
    #[command(alias = "verified_followers")]
    VerifiedFollowers(LimitedUserArgs),
    Subscriptions(LimitedUserArgs),
    #[command(alias = "user_tweets")]
    UserTweets(LimitedUserArgs),
    #[command(alias = "user_tweets_and_replies")]
    UserTweetsAndReplies(LimitedUserArgs),
    #[command(alias = "user_media")]
    UserMedia(LimitedUserArgs),
    #[command(alias = "list_timeline")]
    ListTimeline(ListArgs),
    Trends(TrendArgs),
    Bookmarks(LimitOnlyArgs),
    Doctor(DoctorArgs),
    #[command(alias = "parse_fixture")]
    ParseFixture(ParseFixtureArgs),
}

#[derive(Debug, Args)]
pub struct AddAccountsArgs {
    pub file_path: PathBuf,
    pub line_format: String,
}

#[derive(Debug, Args)]
pub struct AddCookieArgs {
    pub username: String,
    pub cookies: String,
}

#[derive(Debug, Args)]
pub struct UsersArgs {
    pub usernames: Vec<String>,
}

#[derive(Debug, Args, Clone, Copy)]
pub struct LoginArgs {
    #[arg(long)]
    pub email_first: bool,
    #[arg(long)]
    pub manual: bool,
}

#[derive(Debug, Args)]
pub struct ReloginArgs {
    pub usernames: Vec<String>,
    #[arg(long)]
    pub email_first: bool,
    #[arg(long)]
    pub manual: bool,
}

impl From<LoginArgs> for LoginConfig {
    fn from(value: LoginArgs) -> Self {
        Self { email_first: value.email_first, manual: value.manual }
    }
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
    #[arg(long, value_parser = parse_json_arg)]
    pub kv: Option<Value>,
}

#[derive(Debug, Args)]
pub struct TweetArgs {
    pub tweet_id: u64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct LimitedTweetArgs {
    pub tweet_id: u64,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct UserIdArgs {
    pub user_id: u64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct UsernameArgs {
    pub username: String,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct LimitedUserArgs {
    pub user_id: u64,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    pub list_id: u64,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct TrendArgs {
    pub trend_id: String,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct LimitOnlyArgs {
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(subcommand)]
    pub command: DoctorCommand,
}

#[derive(Debug, Subcommand)]
pub enum DoctorCommand {
    Security,
    Imap(DoctorImapArgs),
    Xclid(DoctorXclidArgs),
}

#[derive(Debug, Args)]
pub struct DoctorImapArgs {
    pub email: String,
}

#[derive(Debug, Args)]
pub struct DoctorXclidArgs {
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct ParseFixtureArgs {
    pub file: PathBuf,
    #[arg(value_enum)]
    pub kind: FixtureKind,
    #[arg(long, default_value_t = -1)]
    pub limit: i64,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum FixtureKind {
    Tweets,
    Users,
    Trends,
}

pub async fn run(cli: Cli) -> Result<()> {
    crate::init_tracing(cli.debug);

    let pool = AccountsPool::new(cli.db.clone());
    let api = Api::with_config(
        pool.clone(),
        ApiConfig { proxy: cli.proxy.clone(), base_url: cli.base_url.clone() },
    );

    match cli.command {
        Command::Version => {
            println!("xscraper {}", env!("CARGO_PKG_VERSION"));
            println!("sqlite {}", rusqlite::version());
        }
        Command::Accounts => {
            let rows = pool
                .accounts_info()?
                .into_iter()
                .map(|item| {
                    BTreeMap::from([
                        ("username".into(), item.username),
                        ("logged_in".into(), item.logged_in.to_string()),
                        ("active".into(), item.active.to_string()),
                        (
                            "last_used".into(),
                            item.last_used.map(|dt| dt.to_rfc3339()).unwrap_or_else(|| "-".into()),
                        ),
                        ("total_req".into(), item.total_req.to_string()),
                        ("error_msg".into(), item.error_msg.unwrap_or_default()),
                    ])
                })
                .collect::<Vec<_>>();
            print_table(&rows);
        }
        Command::Stats => {
            let stats = pool.stats()?;
            print_table(&stats.rows());
            println!(
                "Total: {} - Active: {} - Inactive: {}",
                stats.total, stats.active, stats.inactive
            );
        }
        Command::AddAccounts(args) => {
            let added = pool.load_from_file(args.file_path, &args.line_format)?;
            println!("Added {added} account(s). Cookie accounts are active immediately.");
        }
        Command::AddCookie(args) => {
            parse_cookies(&args.cookies)?;
            let added = pool.add_cookie_account(args.username, args.cookies)?;
            println!("{}", if added { "account added" } else { "account already exists" });
        }
        Command::DelAccounts(args) => {
            let deleted = pool.delete_accounts(&args.usernames)?;
            println!("Deleted {deleted} account(s)");
        }
        Command::DeleteInactive => {
            let deleted = pool.delete_inactive()?;
            println!("Deleted {deleted} inactive account(s)");
        }
        Command::LoginAccounts(args) => print_json(&pool.login_all(None, args.into()).await?)?,
        Command::Relogin(args) => {
            let config = LoginConfig { email_first: args.email_first, manual: args.manual };
            print_json(&pool.relogin(&args.usernames, config).await?)?
        }
        Command::ReloginFailed(args) => print_json(&pool.relogin_failed(args.into()).await?)?,
        Command::ResetLocks => {
            pool.reset_locks()?;
            println!("locks reset");
        }
        Command::Search(args) if args.raw => {
            print_pages(api.search_raw(&args.query, args.limit, args.kv).await?)?
        }
        Command::Search(args) => print_items(api.search(&args.query, args.limit, args.kv).await?)?,
        Command::SearchTrend(args) if args.raw => print_pages(
            api.search_raw(
                &args.query,
                args.limit,
                Some(crate::gql::merge_json(
                    serde_json::json!({ "querySource": "trend_click" }),
                    args.kv,
                )),
            )
            .await?,
        )?,
        Command::SearchTrend(args) => {
            print_items(api.search_trend(&args.query, args.limit, args.kv).await?)?
        }
        Command::SearchUser(args) => print_items(api.search_user(&args.query, args.limit).await?)?,
        Command::TweetDetails(args) if args.raw => {
            print_optional(api.tweet_details_raw(args.tweet_id, None).await?)?
        }
        Command::TweetDetails(args) => {
            print_optional(api.tweet_details(args.tweet_id, None).await?)?
        }
        Command::TweetReplies(args) if args.raw => {
            print_pages(api.tweet_replies_raw(args.tweet_id, args.limit, None).await?)?
        }
        Command::TweetReplies(args) => {
            print_items(api.tweet_replies(args.tweet_id, args.limit, None).await?)?
        }
        Command::Retweeters(args) if args.raw => {
            print_pages(api.retweeters_raw(args.tweet_id, args.limit, None).await?)?
        }
        Command::Retweeters(args) => {
            print_items(api.retweeters(args.tweet_id, args.limit, None).await?)?
        }
        Command::UserById(args) if args.raw => {
            print_optional(api.user_by_id_raw(args.user_id, None).await?)?
        }
        Command::UserById(args) => print_optional(api.user_by_id(args.user_id, None).await?)?,
        Command::UserByLogin(args) if args.raw => {
            print_optional(api.user_by_login_raw(&args.username, None).await?)?
        }
        Command::UserByLogin(args) => {
            print_optional(api.user_by_login(&args.username, None).await?)?
        }
        Command::Following(args) if args.raw => {
            print_pages(api.following_raw(args.user_id, args.limit, None).await?)?
        }
        Command::Following(args) => {
            print_items(api.following(args.user_id, args.limit, None).await?)?
        }
        Command::Followers(args) if args.raw => {
            print_pages(api.followers_raw(args.user_id, args.limit, None).await?)?
        }
        Command::Followers(args) => {
            print_items(api.followers(args.user_id, args.limit, None).await?)?
        }
        Command::VerifiedFollowers(args) if args.raw => {
            print_pages(api.verified_followers_raw(args.user_id, args.limit, None).await?)?
        }
        Command::VerifiedFollowers(args) => {
            print_items(api.verified_followers(args.user_id, args.limit, None).await?)?
        }
        Command::Subscriptions(args) if args.raw => {
            print_pages(api.subscriptions_raw(args.user_id, args.limit, None).await?)?
        }
        Command::Subscriptions(args) => {
            print_items(api.subscriptions(args.user_id, args.limit, None).await?)?
        }
        Command::UserTweets(args) if args.raw => {
            print_pages(api.user_tweets_raw(args.user_id, args.limit, None).await?)?
        }
        Command::UserTweets(args) => {
            print_items(api.user_tweets(args.user_id, args.limit, None).await?)?
        }
        Command::UserTweetsAndReplies(args) if args.raw => {
            print_pages(api.user_tweets_and_replies_raw(args.user_id, args.limit, None).await?)?
        }
        Command::UserTweetsAndReplies(args) => {
            print_items(api.user_tweets_and_replies(args.user_id, args.limit, None).await?)?
        }
        Command::UserMedia(args) if args.raw => {
            print_pages(api.user_media_raw(args.user_id, args.limit, None).await?)?
        }
        Command::UserMedia(args) => {
            print_items(api.user_media(args.user_id, args.limit, None).await?)?
        }
        Command::ListTimeline(args) if args.raw => {
            print_pages(api.list_timeline_raw(args.list_id, args.limit, None).await?)?
        }
        Command::ListTimeline(args) => {
            print_items(api.list_timeline(args.list_id, args.limit, None).await?)?
        }
        Command::Trends(args) if args.raw => {
            print_pages(api.trends_raw(&args.trend_id, args.limit, None).await?)?
        }
        Command::Trends(args) => print_items(api.trends(&args.trend_id, args.limit, None).await?)?,
        Command::Bookmarks(args) if args.raw => {
            print_pages(api.bookmarks_raw(args.limit, None).await?)?
        }
        Command::Bookmarks(args) => print_items(api.bookmarks(args.limit, None).await?)?,
        Command::Doctor(args) => run_doctor(args).await?,
        Command::ParseFixture(args) => {
            let raw = std::fs::read_to_string(&args.file)
                .map_err(|source| crate::error::XScraperError::io(args.file.clone(), source))?;
            let value: Value = serde_json::from_str(&raw)?;
            match args.kind {
                FixtureKind::Tweets => {
                    print_items(crate::parser::parse_tweets(&value, args.limit))?
                }
                FixtureKind::Users => print_items(crate::parser::parse_users(&value, args.limit))?,
                FixtureKind::Trends => {
                    print_items(crate::parser::parse_trends(&value, args.limit))?
                }
            }
        }
    }

    Ok(())
}

async fn run_doctor(args: DoctorArgs) -> Result<()> {
    match args.command {
        DoctorCommand::Security => doctor_security()?,
        DoctorCommand::Imap(args) => {
            let host = crate::imap::imap_domain_for_email(&args.email);
            println!("imap: ok email={} host={host}", args.email);
        }
        DoctorCommand::Xclid(args) => doctor_xclid(args).await?,
    }
    Ok(())
}

fn doctor_security() -> Result<()> {
    let tracked = git_ls_files().unwrap_or_default();
    let sensitive =
        tracked.iter().filter(|path| is_sensitive_tracked_path(path)).cloned().collect::<Vec<_>>();
    if !sensitive.is_empty() {
        return Err(crate::error::XScraperError::Config(format!(
            "sensitive files are tracked: {}",
            sensitive.join(", ")
        )));
    }

    let gitignore = std::fs::read_to_string(".gitignore").unwrap_or_default();
    for pattern in ["/accounts.db*", "/.env", "/.env.*", "/.local/"] {
        if !gitignore.lines().any(|line| line.trim() == pattern) {
            return Err(crate::error::XScraperError::Config(format!(
                ".gitignore is missing {pattern}"
            )));
        }
    }

    println!("security: ok sensitive runtime files are ignored and untracked");
    Ok(())
}

fn git_ls_files() -> Option<Vec<String>> {
    let output = ProcessCommand::new("git").arg("ls-files").output().ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    })
}

fn is_sensitive_tracked_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == ".env"
        || name.starts_with(".env.")
        || name == "accounts.db"
        || name.starts_with("accounts.db-")
        || name.ends_with(".sqlite")
        || name.ends_with(".sqlite3")
}

async fn doctor_xclid(args: DoctorXclidArgs) -> Result<()> {
    let id = if args.offline {
        crate::xclid::XClientTransactionIdGenerator::from_parts(vec![1, 2, 3, 4, 5, 6], "abcdef")
            .calc("GET", "/i/api/graphql/test")
    } else {
        let client =
            reqwest::Client::builder().redirect(reqwest::redirect::Policy::limited(10)).build()?;
        crate::xclid::XClientTransactionIdGenerator::create(&client)
            .await?
            .calc("GET", "/i/api/graphql/test")
    };
    println!("xclid: ok len={} sample={}", id.len(), id);
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn print_optional<T: Serialize>(value: Option<T>) -> Result<()> {
    if let Some(value) = value {
        print_json(&value)?;
    } else {
        println!("null");
    }
    Ok(())
}

fn print_items<T: Serialize>(items: Vec<T>) -> Result<()> {
    for item in items {
        print_json(&item)?;
    }
    Ok(())
}

fn print_pages(pages: Vec<Value>) -> Result<()> {
    for page in pages {
        print_json(&page)?;
    }
    Ok(())
}

fn parse_json_arg(raw: &str) -> std::result::Result<Value, String> {
    serde_json::from_str(raw).map_err(|error| error.to_string())
}
