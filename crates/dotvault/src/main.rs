mod add;
mod config;
mod export;
mod resolve;
mod run;
mod set;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use config::DotVaultConfig;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dotvault", about = "Resolve secrets from pluggable backends into your dev environment")]
struct Cli {
    /// Path to config file directory (defaults to current directory)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Resolve secrets and run a subprocess with them injected as env vars
    Run {
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,
    },
    /// Resolve secrets and print `export KEY='VALUE'` lines
    Export,
    /// Create a starter .dotvault.toml in the current directory
    Init,
    /// Print the shell hook snippet for the specified shell
    Hook {
        #[arg(long, value_enum)]
        shell: Shell,
    },
    /// Add a secret mapping to the config file
    Add {
        /// Write to .dotvault.local.toml instead of .dotvault.toml
        #[arg(long)]
        local: bool,
        /// Env var name (interactive if omitted)
        #[arg(long)]
        name: Option<String>,
        /// Provider name (interactive if omitted)
        #[arg(long)]
        provider: Option<String>,
        /// Provider reference/path (interactive if omitted)
        #[arg(long, alias = "ref")]
        reference: Option<String>,
        /// Field name for providers that need it
        #[arg(long)]
        field: Option<String>,
    },
    /// Write a secret value into a vault provider
    Set {
        /// Provider name (interactive if omitted)
        #[arg(long)]
        provider: Option<String>,
        /// Provider reference/path (interactive if omitted)
        #[arg(long, alias = "ref")]
        reference: Option<String>,
        /// Field name for providers that need it
        #[arg(long)]
        field: Option<String>,
        /// Secret value (interactive hidden input if omitted)
        #[arg(long)]
        value: Option<String>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Shell {
    Zsh,
    Bash,
    Fish,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { cmd } => {
            let dir = cli
                .dir
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let mut config = DotVaultConfig::load_from_dir(&dir)?;
            config.merge_global_providers()?;

            let (program, args) = cmd.split_first().expect("cmd must be non-empty");
            run::run_command(&config, program, args).await?;
        }
        Commands::Export => {
            let dir = cli
                .dir
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let mut config = DotVaultConfig::load_from_dir(&dir)?;
            config.merge_global_providers()?;

            let output = export::export_secrets(&config).await?;
            println!("{}", output);
        }
        Commands::Init => {
            let dir = cli
                .dir
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let target = dir.join(".dotvault.toml");
            if target.exists() {
                anyhow::bail!(
                    ".dotvault.toml already exists at {}",
                    target.display()
                );
            }
            let starter = r#"# dotvault configuration
# Declare providers and their configuration here.
# Each provider block is optional; omit if you use defaults.

# [providers.env]
# (no config needed for env provider)

# [providers.aws]
# region = "us-east-1"
# profile = "default"

# Declare secrets below.
# Each secret maps an environment variable name to a provider and reference.

# [secrets.MY_SECRET]
# provider = "env"
# ref = "MY_ENV_VAR"

# [secrets.DB_PASSWORD]
# provider = "aws"
# ref = "sm://my-app/db-password"
"#;
            std::fs::write(&target, starter)?;
            println!("created {}", target.display());
        }
        Commands::Add {
            local,
            name,
            provider,
            reference,
            field,
        } => {
            let dir = cli
                .dir
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            add::add_secret(&dir, local, name, provider, reference, field)?;
        }
        Commands::Set {
            provider,
            reference,
            field,
            value,
        } => {
            let dir = cli
                .dir
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            set::set_secret(&dir, provider, reference, field, value).await?;
        }
        Commands::Hook { shell } => {
            let snippet = match shell {
                Shell::Zsh => {
                    r#"# dotvault zsh hook
# Add this to your ~/.zshrc
dotvault_load() {
  if [ -f .dotvault.toml ] || [ -f .dotvault.local.toml ]; then
    eval "$(dotvault export)"
  fi
}
add-zsh-hook chpwd dotvault_load
dotvault_load"#
                }
                Shell::Bash => {
                    r#"# dotvault bash hook
# Add this to your ~/.bashrc
dotvault_load() {
  if [ -f .dotvault.toml ] || [ -f .dotvault.local.toml ]; then
    eval "$(dotvault export)"
  fi
}
PROMPT_COMMAND="dotvault_load; $PROMPT_COMMAND"
dotvault_load"#
                }
                Shell::Fish => {
                    r#"# dotvault fish hook
# Add this to your ~/.config/fish/config.fish
function dotvault_load --on-variable PWD
  if test -f .dotvault.toml; or test -f .dotvault.local.toml
    dotvault export | source
  end
end
dotvault_load"#
                }
            };
            println!("{}", snippet);
        }
    }

    Ok(())
}
