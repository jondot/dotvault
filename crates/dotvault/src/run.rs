use crate::config::DotVaultConfig;
use crate::resolve::resolve_all;
use anyhow::Result;

pub async fn run_command(config: &DotVaultConfig, cmd: &str, args: &[String]) -> Result<()> {
    let secrets = resolve_all(config).await?;

    let mut command = std::process::Command::new(cmd);
    command.args(args);
    command.envs(&secrets);

    use std::os::unix::process::CommandExt;
    let err = command.exec();
    // exec() only returns on error
    Err(anyhow::anyhow!("failed to exec '{}': {}", cmd, err))
}

