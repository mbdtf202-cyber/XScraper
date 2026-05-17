use clap::Parser;
use xscraper::cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(error) = xscraper::cli::run(cli).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
