use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "test" => run_test(),
        "test-integration" => run_test_integration(),
        "services-up" => services_up(),
        "services-down" => services_down(),
        "ci" => run_ci(),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown task: {other}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        r#"Usage: cargo xtask <task>

Tasks:
  test              Run unit + e2e tests (no external services)
  test-integration  Start services, run all tests including provider integration
  services-up       Start LocalStack + Vault via docker compose
  services-down     Stop test services
  ci                Full CI: services + all tests"#
    );
}

/// Unit + e2e tests, no external services needed.
fn run_test() -> ExitCode {
    let steps: &[&[&str]] = &[
        &["cargo", "test", "-p", "dotvault"],
        &["cargo", "test", "-p", "secret-resolvers", "--features", "all"],
    ];
    run_steps(steps)
}

/// Full integration: start services, wait, run all tests.
fn run_test_integration() -> ExitCode {
    if services_up() != ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }
    if !wait_for("LocalStack", "http://localhost:4566/_localstack/health") {
        return ExitCode::FAILURE;
    }
    if !wait_for("Vault", "http://localhost:8200/v1/sys/health") {
        return ExitCode::FAILURE;
    }
    enable_vault_kv();
    run_test()
}

fn run_ci() -> ExitCode {
    run_test_integration()
}

fn docker_compose() -> Vec<String> {
    // Try `docker compose` (v2 plugin) first, fall back to `docker-compose` (standalone)
    if Command::new("docker")
        .args(["compose", "version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        vec!["docker".into(), "compose".into()]
    } else {
        vec!["docker-compose".into()]
    }
}

fn services_up() -> ExitCode {
    let mut cmd = docker_compose();
    cmd.extend(["-f", "docker-compose.test.yml", "up", "-d"].map(String::from));
    let refs: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
    run_steps(&[&refs])
}

fn services_down() -> ExitCode {
    let mut cmd = docker_compose();
    cmd.extend(["-f", "docker-compose.test.yml", "down"].map(String::from));
    let refs: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
    run_steps(&[&refs])
}

fn wait_for(name: &str, url: &str) -> bool {
    eprint!("Waiting for {name}...");
    for _ in 0..30 {
        if Command::new("curl")
            .args(["-sf", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!(" ready");
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        eprint!(".");
    }
    eprintln!(" timeout after 30s");
    false
}

fn enable_vault_kv() {
    // Vault dev mode has kv v2 at secret/ by default, but ensure it's there
    let _ = Command::new("curl")
        .args([
            "-sf",
            "-X", "POST",
            "http://localhost:8200/v1/sys/mounts/secret",
            "-H", "X-Vault-Token: test-root-token",
            "-d", r#"{"type":"kv","options":{"version":"2"}}"#,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn run_steps(steps: &[&[&str]]) -> ExitCode {
    for step in steps {
        eprintln!("→ {}", step.join(" "));
        let status = Command::new(step[0])
            .args(&step[1..])
            .status()
            .unwrap_or_else(|e| {
                eprintln!("failed to run '{}': {e}", step[0]);
                std::process::exit(1);
            });
        if !status.success() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
