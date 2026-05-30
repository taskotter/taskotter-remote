use std::path::PathBuf;

use clap::{Parser, Subcommand};
use taskotter_remote::{
    protocol::{RunnerEnvelope, RunnerEvent, RunnerState},
    Config, Daemon,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        #[arg(long, default_value = "examples/runner.toml")]
        config: PathBuf,
    },
    ValidateConfig {
        #[arg(long, default_value = "examples/runner.toml")]
        config: PathBuf,
    },
    Capabilities {
        #[arg(long, default_value = "examples/runner.toml")]
        config: PathBuf,
    },
    Protocol,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    match Cli::parse().command {
        Command::Run { config } => {
            let config = Config::from_path(config).await?;
            Daemon::new(config).run_until_shutdown().await?;
        }
        Command::ValidateConfig { config } => {
            let config = Config::from_path(config).await?;
            println!(
                "config ok: runner={} token_env={} token_present={}",
                config.runner_name,
                config.control_plane.registration_token_env,
                config.registration_token_is_available()
            );
        }
        Command::Capabilities { config } => {
            let config = Config::from_path(config).await?;
            let daemon = Daemon::new(config);
            println!(
                "{}",
                serde_json::to_string_pretty(&daemon.capability_inventory())?
            );
        }
        Command::Protocol => {
            let config = Config::from_path("examples/runner.toml").await?;
            let daemon = Daemon::new(config);
            let envelope = RunnerEnvelope::new(
                daemon.runner_id(),
                1,
                RunnerEvent::Status(daemon.status(RunnerState::Starting)),
            );
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
    }

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
