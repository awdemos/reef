mod cli;
mod server;
mod worker;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    let log_level = if cli.debug { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("reef= {},tower_http=debug", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    cli.execute().await
}
