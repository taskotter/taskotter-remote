use anyhow::Context;
use clap::{Parser, Subcommand};
use taskotter_remote::{
    capabilities::RunnerCapabilities, config::RemoteConfig, daemon::RunnerDaemon,
};

#[derive(Debug, Parser)]
#[command(name = "taskotter-remote")]
#[command(about = "TaskOtter Remote runner/daemon MVP foundation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the runner capability inventory as versioned JSON.
    PrintCapabilities {
        #[arg(short, long, default_value = "examples/remote.toml")]
        config: String,
    },
    /// Print the registration request payload as versioned JSON.
    Register {
        #[arg(short, long, default_value = "examples/remote.toml")]
        config: String,
    },
    /// Run one placeholder daemon lifecycle and print emitted contracts.
    RunOnce {
        #[arg(short, long, default_value = "examples/remote.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::PrintCapabilities { config } => {
            let config = load_config(config)?;
            let capabilities = RunnerCapabilities::discover(&config);
            println!("{}", serde_json::to_string_pretty(&capabilities)?);
        }
        Command::Register { config } => {
            let daemon = RunnerDaemon::new(load_config(config)?);
            println!(
                "{}",
                serde_json::to_string_pretty(&daemon.registration_payload())?
            );
        }
        Command::RunOnce { config } => {
            let daemon = RunnerDaemon::new(load_config(config)?);
            for envelope in daemon.run_once().await? {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            }
        }
    }

    Ok(())
}

fn load_config(path: String) -> anyhow::Result<RemoteConfig> {
    RemoteConfig::from_path(&path).with_context(|| format!("failed to load config at {path}"))
}
