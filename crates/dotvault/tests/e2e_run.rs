use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn write_config(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[test]
fn test_run_injects_env_vars() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.INJECTED_SECRET]
provider = "env"
ref = "DOTVAULT_RUN_SRC"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "run",
        "--",
        "env",
    ])
    .env("DOTVAULT_RUN_SRC", "injected_value");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("INJECTED_SECRET=injected_value"));
}

#[test]
fn test_run_passes_arguments_to_subprocess() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.ANOTHER_SECRET]
provider = "env"
ref = "DOTVAULT_ARG_SRC"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "run",
        "--",
        "sh",
        "-c",
        "echo $ANOTHER_SECRET",
    ])
    .env("DOTVAULT_ARG_SRC", "arg_value");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("arg_value"));
}
