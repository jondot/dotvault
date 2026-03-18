use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn write_config(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[test]
fn test_no_config_file_error() {
    let dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("no config file found"));
}

#[test]
fn test_unknown_provider_error() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.SECRET]
provider = "totally_unknown_provider_xyz"
ref = "something"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unknown provider"));
}

#[test]
fn test_missing_env_var_error() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        ".dotvault.toml",
        r#"
[secrets.MISSING]
provider = "env"
ref = "DOTVAULT_DEFINITELY_NOT_SET_VARIABLE_XYZ_12345"
"#,
    );

    let mut cmd = Command::cargo_bin("dotvault").unwrap();
    cmd.args(["--dir", dir.path().to_str().unwrap(), "export"])
        .env_remove("DOTVAULT_DEFINITELY_NOT_SET_VARIABLE_XYZ_12345");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("resolution failed"));
}
