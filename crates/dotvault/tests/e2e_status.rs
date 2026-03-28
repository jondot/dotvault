use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use serde_json;

fn write_config(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[test]
fn test_status_all_ok() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_OK"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "status"])
        .env("DOTVAULT_STATUS_OK", "hello1234");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("MY_SECRET"))
        .stderr(predicate::str::contains("env"))
        .stderr(predicate::str::contains("\u{2713}"))
        .stderr(predicate::str::contains("****1234"));
}

#[test]
fn test_status_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MISSING_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_NOPE"
"#,
    );

    // Do not set the env var — resolution should fail
    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "status"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("MISSING_SECRET"))
        .stderr(predicate::str::contains("\u{2717}"));
}

#[test]
fn test_status_masked_short_value() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.SHORT_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_SHORT"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "status"])
        .env("DOTVAULT_STATUS_SHORT", "ab");

    // Value is <= 4 chars, should show only "****"
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("****"), "short value should be masked, got: {stderr}");
    assert!(
        !stderr.contains("ab"),
        "short value should not be revealed, got: {stderr}"
    );
}

#[test]
fn test_status_with_only_filter() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.INCLUDED]
provider = "env"
ref = "DOTVAULT_STATUS_INC"

[secrets.EXCLUDED]
provider = "env"
ref = "DOTVAULT_STATUS_EXC"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "status",
        "--only",
        "INCLUDED",
    ])
    .env("DOTVAULT_STATUS_INC", "included_val")
    .env("DOTVAULT_STATUS_EXC", "excluded_val");

    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("INCLUDED"), "filtered secret should appear, got: {stderr}");
    assert!(
        !stderr.contains("EXCLUDED"),
        "non-filtered secret should not appear, got: {stderr}"
    );
}

#[test]
fn test_status_shows_header() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_HDR"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "status"])
        .env("DOTVAULT_STATUS_HDR", "headerval");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Environment:"))
        .stderr(predicate::str::contains("Config: .dotvault.toml"));
}

#[test]
fn test_status_mixed_results() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.GOOD_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_GOOD"

[secrets.BAD_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_BAD"
"#,
    );

    // Only set one of the two
    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "status"])
        .env("DOTVAULT_STATUS_GOOD", "good_value_here");

    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should exit non-zero because one secret fails
    assert!(!output.status.success(), "should exit non-zero with failures, got: {stderr}");
    // Good secret should show checkmark and masked value
    assert!(stderr.contains("\u{2713}"), "good secret should have checkmark, got: {stderr}");
    assert!(stderr.contains("****here"), "good secret should show masked value, got: {stderr}");
    // Bad secret should show X
    assert!(stderr.contains("\u{2717}"), "bad secret should have X, got: {stderr}");
}

#[test]
fn test_status_json_all_ok() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_SECRET]
provider = "env"
ref = "DOTVAULT_STATUS_JSON_OK"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "status",
        "--format",
        "json",
    ])
    .env("DOTVAULT_STATUS_JSON_OK", "hello1234");

    let output = cmd.output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    assert_eq!(parsed["all_ok"], true);
    assert!(parsed["environment"].is_string());

    let secrets = parsed["secrets"].as_array().unwrap();
    assert_eq!(secrets.len(), 1);
    assert_eq!(secrets[0]["name"], "MY_SECRET");
    assert_eq!(secrets[0]["provider"], "env");
    assert_eq!(secrets[0]["status"], "ok");
}

#[test]
fn test_status_json_with_failure() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.GOOD]
provider = "env"
ref = "DOTVAULT_STATUS_JSON_GOOD"

[secrets.BAD]
provider = "env"
ref = "DOTVAULT_STATUS_JSON_BAD"
"#,
    );

    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.args([
        "--dir",
        dir.path().to_str().unwrap(),
        "status",
        "--format",
        "json",
    ])
    .env("DOTVAULT_STATUS_JSON_GOOD", "good_value");

    let output = cmd.output().unwrap();
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    assert_eq!(parsed["all_ok"], false);

    let secrets = parsed["secrets"].as_array().unwrap();
    let good = secrets.iter().find(|s| s["name"] == "GOOD").unwrap();
    assert_eq!(good["status"], "ok");

    let bad = secrets.iter().find(|s| s["name"] == "BAD").unwrap();
    assert_eq!(bad["status"], "error");
    assert!(bad["error"].is_string());
}
