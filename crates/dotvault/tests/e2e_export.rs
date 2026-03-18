use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn write_config(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[test]
fn test_export_with_env_provider() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MY_EXPORTED_VAR]
provider = "env"
ref = "DOTVAULT_TEST_SRC_VAR"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"])
        .env("DOTVAULT_TEST_SRC_VAR", "hello_export");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("export MY_EXPORTED_VAR='hello_export'"));
}

#[test]
fn test_export_local_replaces_shared() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.TEST_KEY]
provider = "env"
ref = "DOTVAULT_SHARED_VAR"
"#,
    );
    write_config(
        &dir,
        ".dotvault.local.toml",
        r#"
[secrets.TEST_KEY]
provider = "env"
ref = "DOTVAULT_LOCAL_VAR"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"])
        .env("DOTVAULT_LOCAL_VAR", "from_local")
        .env_remove("DOTVAULT_SHARED_VAR");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("export TEST_KEY='from_local'"));
}

#[test]
fn test_export_escapes_single_quotes() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.QUOTED_VAR]
provider = "env"
ref = "DOTVAULT_QUOTE_SRC"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"])
        .env("DOTVAULT_QUOTE_SRC", "it's a test");

    // Single quote in value should be escaped as '\''
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("export QUOTED_VAR='it'\\''s a test'"));
}
