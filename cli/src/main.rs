use anyhow::Result;
use clap::Parser;

mod commands;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = commands::Cli::parse();
    commands::execute(cli).await
}
