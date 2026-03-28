use crate::config::DotVaultConfig;
use crate::resolve::resolve_all;
use anyhow::Result;
use secret_resolvers::ExposeSecret;

const ESSENTIAL_VARS: &[&str] = &[
    "PATH", "HOME", "USER", "SHELL", "TERM", "LANG", "LC_ALL", "TMPDIR", "TZ",
];

pub async fn run_command(
    config: &DotVaultConfig,
    cmd: &str,
    args: &[String],
    only: Option<&[String]>,
    clean_env: bool,
    keep_env: Option<&[String]>,
) -> Result<()> {
    let secrets = resolve_all(config, only).await?;

    let mut command = std::process::Command::new(cmd);
    command.args(args);

    if clean_env {
        command.env_clear();

        // Re-inject essential vars from current environment
        for &var in ESSENTIAL_VARS {
            if let Ok(val) = std::env::var(var) {
                command.env(var, val);
            }
        }

        // Re-inject user-specified keep vars
        if let Some(keep) = keep_env {
            for var in keep {
                if let Ok(val) = std::env::var(var) {
                    command.env(var, val);
                }
            }
        }
    }

    // Secrets go last so they override on collision
    command.envs(secrets.iter().map(|(k, v)| (k, v.expose_secret())));

    use std::os::unix::process::CommandExt;
    let err = command.exec();
    // exec() only returns on error
    Err(anyhow::anyhow!("failed to exec '{}': {}", cmd, err))
}
