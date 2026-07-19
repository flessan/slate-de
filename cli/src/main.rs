use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "config/slate.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    tracing_subscriber::fmt::init();
    
    tracing::info!("Starting Slate-DE...");
    
    Ok(())
}
