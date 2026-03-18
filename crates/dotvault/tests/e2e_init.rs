use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_init_creates_file() {
    let dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "init"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("created"));

    let config_path = dir.path().join(".dotvault.toml");
    assert!(config_path.exists(), ".dotvault.toml should have been created");

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("dotvault configuration"), "file should contain starter content");
}

#[test]
fn test_init_errors_if_exists() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join(".dotvault.toml");
    std::fs::write(&config_path, "# existing config\n").unwrap();

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "init"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
