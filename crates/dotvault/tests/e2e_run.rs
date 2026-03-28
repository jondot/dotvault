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

    let mut cmd = Command::cargo_bin("dv").unwrap();
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

    let mut cmd = Command::cargo_bin("dv").unwrap();
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

#[test]
fn test_run_clean_env_removes_non_essential_vars() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_CLEAN_SRC"
"#,
    );

    // Set a non-essential var that should be stripped
    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "run",
        "--clean-env",
        "--",
        "env",
    ])
    .env("DOTVAULT_CLEAN_SRC", "secret_val")
    .env("SHOULD_BE_GONE", "leaked");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Secret should be present
    assert!(
        stdout.contains("MY_SECRET=secret_val"),
        "secret should be injected, got: {stdout}"
    );
    // Non-essential var should be gone
    assert!(
        !stdout.contains("SHOULD_BE_GONE"),
        "non-essential var should be cleared, got: {stdout}"
    );
    // PATH should still be present (essential)
    assert!(
        stdout.contains("PATH="),
        "PATH should be preserved, got: {stdout}"
    );
}

#[test]
fn test_run_clean_env_with_keep_env() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_KEEP_SRC"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "run",
        "--clean-env",
        "--keep-env",
        "CUSTOM_KEPT",
        "--",
        "env",
    ])
    .env("DOTVAULT_KEEP_SRC", "secret_val")
    .env("CUSTOM_KEPT", "preserved")
    .env("CUSTOM_DROPPED", "gone");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("MY_SECRET=secret_val"),
        "secret should be injected, got: {stdout}"
    );
    assert!(
        stdout.contains("CUSTOM_KEPT=preserved"),
        "kept var should be preserved, got: {stdout}"
    );
    assert!(
        !stdout.contains("CUSTOM_DROPPED"),
        "non-kept var should be cleared, got: {stdout}"
    );
}

#[test]
fn test_run_without_clean_env_inherits_all() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_INHERIT_SRC"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "run",
        "--",
        "env",
    ])
    .env("DOTVAULT_INHERIT_SRC", "secret_val")
    .env("RANDOM_VAR", "should_exist");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("MY_SECRET=secret_val"),
        "secret should be injected, got: {stdout}"
    );
    assert!(
        stdout.contains("RANDOM_VAR=should_exist"),
        "inherited var should be present without --clean-env, got: {stdout}"
    );
}
