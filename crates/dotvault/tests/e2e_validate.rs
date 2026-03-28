use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn write_config(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[test]
fn test_validate_valid_config() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.API_KEY]
provider = "env"
ref = "MY_KEY"

[secrets.DB_URL]
provider = "keychain"
ref = "db-url"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "validate"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ok"))
        .stdout(predicate::str::contains("2 secrets"));
}

#[test]
fn test_validate_unknown_provider() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.BAD]
provider = "nonexistent"
ref = "something"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "validate"]);

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("unknown provider"));
}

#[test]
fn test_validate_no_config() {
    let dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "validate"]);

    cmd.assert().failure();
}

#[test]
fn test_validate_invalid_toml() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, ".dotvault.toml", "this is [[[not valid toml");

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "validate"]);

    cmd.assert().failure();
}

#[test]
fn test_validate_json_format() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.API_KEY]
provider = "env"
ref = "MY_KEY"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "validate",
        "--format",
        "json",
    ]);

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    assert_eq!(parsed["valid"], true);
    assert_eq!(parsed["secrets_count"], 1);
    assert_eq!(parsed["errors"].as_array().unwrap().len(), 0);
}

#[test]
fn test_validate_json_format_with_errors() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.GOOD]
provider = "env"
ref = "X"

[secrets.BAD]
provider = "fakeprovider"
ref = "Y"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "validate",
        "--format",
        "json",
    ]);

    let output = cmd.output().unwrap();
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    assert_eq!(parsed["valid"], false);
    let errors = parsed["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["name"], "BAD");
}

#[test]
fn test_validate_named_provider_accepted() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[providers.my-custom]
type = "env"

[secrets.API_KEY]
provider = "my-custom"
ref = "X"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "validate"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}
