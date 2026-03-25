# dotvault Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that resolves secrets from pluggable backends and injects them into developer environments.

**Architecture:** Cargo workspace with two crates: `secret-resolvers` (shared provider library with trait + 8 providers behind feature flags) and `dotvault` (CLI binary using clap). Distribution via npm platform packages, cargo, and GitHub Releases — adapted from the tooly project.

**Tech Stack:** Rust, clap, tokio, async-trait, toml, serde. Provider crates: keyring, age, aws-sdk-*, vaultrs, google-cloud-secretmanager-v1, reqwest. Testing: assert_cmd, predicates, wiremock, Docker Compose (LocalStack, Vault dev).

**Reference:** Spec at `docs/superpowers/specs/2026-03-18-dotvault-design.md`. Distribution files adapted from `~/projects/tooly`.

---

## File Structure

### `secret-resolvers` crate (`crates/secret-resolvers/`)

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest with feature flags per provider |
| `src/lib.rs` | Re-exports trait, types, provider modules |
| `src/provider.rs` | `SecretResolver` trait, `ResolveRequest`, `ResolvedSecret`, `ResolverError` |
| `src/env.rs` | `EnvResolver` — reads from environment variables |
| `src/onepassword.rs` | `OnePasswordResolver` — shells out to `op` CLI |
| `src/keychain.rs` | `KeychainResolver` — uses `keyring` crate |
| `src/age.rs` | `AgeResolver` — decrypts age-encrypted files |
| `src/aws.rs` | `AwsResolver` — AWS Secrets Manager + SSM Parameter Store |
| `src/hashicorp.rs` | `HashiCorpResolver` — HashiCorp Vault via `vaultrs` |
| `src/gcp.rs` | `GcpResolver` — Google Cloud Secret Manager |
| `src/keyzero.rs` | `KeyzeroResolver` — HTTP client to keyzero `/v1/resolve` |

### `dotvault` crate (`crates/dotvault/`)

| File | Responsibility |
|---|---|
| `Cargo.toml` | Binary crate, depends on `secret-resolvers` with all features |
| `src/main.rs` | clap CLI definition, subcommand dispatch |
| `src/config.rs` | Parse `.dotvault.toml` / `.dotvault.local.toml`, merge provider config with global config |
| `src/resolve.rs` | Orchestrate: load config → instantiate providers → resolve concurrently → return HashMap |
| `src/run.rs` | `dotvault run -- <cmd>` — exec subprocess with env vars |
| `src/export.rs` | `dotvault export` — print `export KEY="VALUE"` lines |

### Workspace root

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace manifest |
| `.gitignore` | Standard Rust + `.dotvault.local.toml` |
| `shell/hook.zsh` | zsh chpwd hook |
| `shell/hook.bash` | bash PROMPT_COMMAND hook |
| `shell/hook.fish` | fish hook |
| `docker-compose.test.yml` | LocalStack + Vault dev for integration tests |

### Distribution (adapted from ~/projects/tooly)

| File | Responsibility |
|---|---|
| `npm/dotvault/package.json` | Umbrella npm package with optionalDependencies |
| `npm/dotvault/bin/dotvault` | Node.js wrapper (platform detection + binary exec) |
| `npm/cli-darwin-arm64/package.json` | macOS ARM64 platform package |
| `npm/cli-linux-x64/package.json` | Linux x64 platform package |
| `npm/cli-linux-arm64/package.json` | Linux ARM64 platform package |
| `.github/workflows/release.yml` | Build matrix + npm publish + GitHub Release |
| `scripts/bump-version.sh` | Coordinated version bump |

### Tests

| File | Responsibility |
|---|---|
| `crates/secret-resolvers/tests/integration_env.rs` | Integration test for env provider |
| `crates/secret-resolvers/tests/integration_keychain.rs` | Integration test for keychain provider (mock store) |
| `crates/secret-resolvers/tests/integration_age.rs` | Integration test for age provider |
| `crates/secret-resolvers/tests/integration_onepassword.rs` | Integration test for 1Password provider |
| `crates/secret-resolvers/tests/integration_aws.rs` | Integration test for AWS provider (LocalStack) |
| `crates/secret-resolvers/tests/integration_hashicorp.rs` | Integration test for HashiCorp Vault (dev server) |
| `crates/secret-resolvers/tests/integration_gcp.rs` | Integration test for GCP provider (wiremock) |
| `crates/secret-resolvers/tests/integration_keyzero.rs` | Integration test for keyzero provider (wiremock) |
| `crates/dotvault/tests/e2e_run.rs` | E2E test for `dotvault run` command |
| `crates/dotvault/tests/e2e_export.rs` | E2E test for `dotvault export` command |
| `crates/dotvault/tests/e2e_init.rs` | E2E test for `dotvault init` command |
| `crates/dotvault/tests/e2e_errors.rs` | E2E test for error cases |

---

## Task 1: Workspace Scaffold + secret-resolvers Trait

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/secret-resolvers/Cargo.toml`
- Create: `crates/secret-resolvers/src/lib.rs`
- Create: `crates/secret-resolvers/src/provider.rs`
- Create: `crates/dotvault/Cargo.toml`
- Create: `crates/dotvault/src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
members = ["crates/dotvault", "crates/secret-resolvers"]
resolver = "2"
```

- [ ] **Step 2: Create secret-resolvers Cargo.toml**

```toml
[package]
name = "secret-resolvers"
version = "0.1.0"
edition = "2021"
description = "Pluggable secret resolution from multiple backends"
license = "MIT"

[dependencies]
async-trait = "0.1"
toml = "0.8"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["process"] }

# Provider-specific dependencies (behind feature flags)
keyring = { version = "3", optional = true, features = ["apple-native", "linux-native"] }
age = { version = "0.11", optional = true }
aws-config = { version = "1", optional = true }
aws-sdk-secretsmanager = { version = "1", optional = true }
aws-sdk-ssm = { version = "1", optional = true }
reqwest = { version = "0.12", features = ["json"], optional = true }

[features]
default = ["env"]
env = []
onepassword = []
keychain = ["dep:keyring"]
age-provider = ["dep:age"]
aws = ["dep:aws-config", "dep:aws-sdk-secretsmanager", "dep:aws-sdk-ssm"]
hashicorp = ["dep:reqwest"]
gcp = ["dep:reqwest"]
keyzero = ["dep:reqwest"]
all = ["env", "onepassword", "keychain", "age-provider", "aws", "hashicorp", "gcp", "keyzero"]
```

Note: GCP uses reqwest directly (calling the REST API) rather than the heavy `google-cloud-secretmanager-v1` crate — keeps the dependency tree smaller and avoids pulling in gRPC/tonic. The REST API surface we need (access a secret version) is one endpoint.

- [ ] **Step 3: Create provider.rs with trait, types, and error**

```rust
// crates/secret-resolvers/src/provider.rs
use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("missing required param '{0}'")]
    MissingParam(String),

    #[error("resolution failed: {0}")]
    ResolutionFailed(String),

    #[error("configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, ResolverError>;

pub struct ResolveRequest {
    pub params: HashMap<String, toml::Value>,
}

impl ResolveRequest {
    pub fn get_str(&self, key: &str) -> Result<&str> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam(key.to_string()))
    }

    pub fn get_str_opt(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(|v| v.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedSecret {
    pub value: String,
    pub ttl: Option<u64>,
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret>;
}
```

- [ ] **Step 4: Create lib.rs with module declarations**

```rust
// crates/secret-resolvers/src/lib.rs
mod provider;
pub use provider::*;

#[cfg(feature = "env")]
pub mod env;

#[cfg(feature = "onepassword")]
pub mod onepassword;

#[cfg(feature = "keychain")]
pub mod keychain;

#[cfg(feature = "age-provider")]
pub mod age;

#[cfg(feature = "aws")]
pub mod aws;

#[cfg(feature = "hashicorp")]
pub mod hashicorp;

#[cfg(feature = "gcp")]
pub mod gcp;

#[cfg(feature = "keyzero")]
pub mod keyzero;
```

- [ ] **Step 5: Create stub dotvault crate**

`crates/dotvault/Cargo.toml`:
```toml
[package]
name = "dotvault"
version = "0.1.0"
edition = "2021"
description = "Resolve secrets from pluggable backends into your dev environment"
license = "MIT"

[[bin]]
name = "dotvault"
path = "src/main.rs"

[dependencies]
secret-resolvers = { path = "../secret-resolvers", features = ["all"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
dirs = "6"
anyhow = "1"
futures = "0.3"
```

`crates/dotvault/src/main.rs`:
```rust
fn main() {
    println!("dotvault stub");
}
```

- [ ] **Step 6: Create .gitignore**

```
/target
.dotvault.local.toml
*.age
```

- [ ] **Step 7: Verify workspace builds**

Run: `cargo build`
Expected: compiles successfully with no errors.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/ .gitignore
git commit -m "feat: scaffold workspace with secret-resolvers trait and dotvault stub"
```

---

## Task 2: Env Provider + Integration Test

**Files:**
- Create: `crates/secret-resolvers/src/env.rs`
- Create: `crates/secret-resolvers/tests/integration_env.rs`

- [ ] **Step 1: Write integration test**

```rust
// crates/secret-resolvers/tests/integration_env.rs
use secret_resolvers::{EnvResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

#[tokio::test]
async fn test_env_resolves_existing_var() {
    std::env::set_var("DOTVAULT_TEST_SECRET", "hello-world");
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String("DOTVAULT_TEST_SECRET".to_string()))]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "hello-world");
    assert_eq!(result.ttl, None);
}

#[tokio::test]
async fn test_env_errors_on_missing_var() {
    std::env::remove_var("DOTVAULT_NONEXISTENT_VAR");
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String("DOTVAULT_NONEXISTENT_VAR".to_string()))]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_env_errors_on_missing_ref_param() {
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::new(),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_env`
Expected: FAIL — `EnvResolver` not found.

- [ ] **Step 3: Implement EnvResolver**

```rust
// crates/secret-resolvers/src/env.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct EnvResolver;

impl EnvResolver {
    pub fn new(_config: HashMap<String, toml::Value>) -> Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl SecretResolver for EnvResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let var_name = request.get_str("ref")?;
        let value = std::env::var(var_name).map_err(|_| {
            ResolverError::ResolutionFailed(format!("environment variable '{var_name}' not set"))
        })?;
        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Update `lib.rs` to add `pub use env::EnvResolver;` under the `#[cfg(feature = "env")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_env`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/env.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/
git commit -m "feat: add env provider with integration tests"
```

---

## Task 3: 1Password Provider + Integration Test

**Files:**
- Create: `crates/secret-resolvers/src/onepassword.rs`
- Create: `crates/secret-resolvers/tests/integration_onepassword.rs`

- [ ] **Step 1: Write integration test**

The 1Password provider shells out to `op`. For integration testing, we inject a fake `op` script via the constructor config.

```rust
// crates/secret-resolvers/tests/integration_onepassword.rs
use secret_resolvers::{OnePasswordResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use std::io::Write;

fn create_fake_op(response_json: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".sh").tempfile().unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo '{}'", response_json).unwrap();
    let path = f.path().to_owned();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    f
}

fn create_failing_op() -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".sh").tempfile().unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo 'item not found' >&2").unwrap();
    writeln!(f, "exit 1").unwrap();
    let path = f.path().to_owned();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    f
}

#[tokio::test]
async fn test_onepassword_resolves_secret() {
    let fake_op = create_fake_op(r#"{"value": "sk-test-12345"}"#);
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(fake_op.path().to_str().unwrap().to_string())),
    ]);
    let resolver = OnePasswordResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("op://Engineering/OpenAI/api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "sk-test-12345");
}

#[tokio::test]
async fn test_onepassword_errors_on_failed_op() {
    let fake_op = create_failing_op();
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(fake_op.path().to_str().unwrap().to_string())),
    ]);
    let resolver = OnePasswordResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("op://Vault/Item/field".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_onepassword`
Expected: FAIL — `OnePasswordResolver` not found.

- [ ] **Step 3: Implement OnePasswordResolver**

```rust
// crates/secret-resolvers/src/onepassword.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct OnePasswordResolver {
    op_path: String,
    account: Option<String>,
    vault: Option<String>,
}

impl OnePasswordResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        Ok(Self {
            op_path: config
                .get("op_path")
                .and_then(|v| v.as_str())
                .unwrap_or("op")
                .to_string(),
            account: config.get("account").and_then(|v| v.as_str()).map(String::from),
            vault: config.get("vault").and_then(|v| v.as_str()).map(String::from),
        })
    }
}

#[async_trait]
impl SecretResolver for OnePasswordResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let reference = request.get_str("ref")?;

        let mut cmd = tokio::process::Command::new(&self.op_path);
        cmd.args(["read", reference]);
        if let Some(account) = &self.account {
            cmd.args(["--account", account]);
        }
        if let Some(vault) = &self.vault {
            cmd.args(["--vault", vault]);
        }

        let output = cmd.output().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("failed to run op CLI: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ResolverError::ResolutionFailed(format!("op CLI: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let value = stdout.trim().to_string();

        // op read returns the value directly, but some older flows return JSON
        // Try JSON parse first, fall back to raw string
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) {
            if let Some(v) = parsed.get("value").and_then(|v| v.as_str()) {
                return Ok(ResolvedSecret { value: v.to_string(), ttl: None });
            }
        }

        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Update `lib.rs` to add `pub use onepassword::OnePasswordResolver;` under the `#[cfg(feature = "onepassword")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_onepassword`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/onepassword.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/
git commit -m "feat: add 1Password provider with integration tests"
```

---

## Task 4: Keychain Provider + Integration Test

**Files:**
- Create: `crates/secret-resolvers/src/keychain.rs`
- Create: `crates/secret-resolvers/tests/integration_keychain.rs`

- [ ] **Step 1: Write integration test**

The `keyring` crate has a `mock` feature for testing. We use the `keyring::mock` credential store.

```rust
// crates/secret-resolvers/tests/integration_keychain.rs
use secret_resolvers::{KeychainResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

#[tokio::test]
async fn test_keychain_resolves_stored_secret() {
    // Use keyring mock to store a test credential
    let entry = keyring::Entry::new("dotvault-test", "test-key").unwrap();
    entry.set_password("my-secret-value").unwrap();

    let resolver = KeychainResolver::new(HashMap::from([
        ("service".to_string(), toml::Value::String("dotvault-test".to_string())),
    ])).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("test-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "my-secret-value");

    // Cleanup
    entry.delete_credential().unwrap();
}

#[tokio::test]
async fn test_keychain_errors_on_missing_secret() {
    let resolver = KeychainResolver::new(HashMap::from([
        ("service".to_string(), toml::Value::String("dotvault-test-nonexistent".to_string())),
    ])).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("does-not-exist".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_keychain --features keychain`
Expected: FAIL — `KeychainResolver` not found.

- [ ] **Step 3: Implement KeychainResolver**

```rust
// crates/secret-resolvers/src/keychain.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct KeychainResolver {
    service: String,
}

impl KeychainResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let service = config
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("dotvault")
            .to_string();
        Ok(Self { service })
    }
}

#[async_trait]
impl SecretResolver for KeychainResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let item_name = request.get_str("ref")?;
        let entry = keyring::Entry::new(&self.service, item_name)
            .map_err(|e| ResolverError::ResolutionFailed(format!("keychain entry error: {e}")))?;
        let value = entry.get_password().map_err(|e| {
            ResolverError::ResolutionFailed(format!("keychain lookup failed for '{item_name}': {e}"))
        })?;
        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Update `lib.rs` to add `pub use keychain::KeychainResolver;` under the `#[cfg(feature = "keychain")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_keychain --features keychain`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/keychain.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/
git commit -m "feat: add keychain provider with integration tests"
```

---

## Task 5: Age Provider + Integration Test

**Files:**
- Create: `crates/secret-resolvers/src/age.rs`
- Create: `crates/secret-resolvers/tests/integration_age.rs`

- [ ] **Step 1: Write integration test**

Tests generate a real age keypair, encrypt a secret, and resolve it. Pure crypto, no external deps.

```rust
// crates/secret-resolvers/tests/integration_age.rs
use secret_resolvers::{AgeResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use std::io::Write;

fn setup_age_encrypted_file(plaintext: &str) -> (tempfile::NamedTempFile, tempfile::NamedTempFile) {
    use age::secrecy::ExposeSecret;

    let identity = age::x25519::Identity::generate();
    let recipient = identity.to_public();

    // Write identity (private key) to a temp file
    let mut identity_file = tempfile::NamedTempFile::new().unwrap();
    writeln!(identity_file, "{}", identity.to_string().expose_secret()).unwrap();

    // Encrypt plaintext to a temp file
    let encrypted_file = tempfile::NamedTempFile::new().unwrap();
    let encryptor = age::Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
    let mut writer = encryptor.wrap_output(std::fs::File::create(encrypted_file.path()).unwrap()).unwrap();
    writer.write_all(plaintext.as_bytes()).unwrap();
    writer.finish().unwrap();

    (identity_file, encrypted_file)
}

#[tokio::test]
async fn test_age_resolves_encrypted_file() {
    let (identity_file, encrypted_file) = setup_age_encrypted_file("super-secret-value");

    let config = HashMap::from([
        ("identity".to_string(), toml::Value::String(identity_file.path().to_str().unwrap().to_string())),
    ]);
    let resolver = AgeResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String(encrypted_file.path().to_str().unwrap().to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "super-secret-value");
}

#[tokio::test]
async fn test_age_errors_on_missing_file() {
    let (identity_file, _) = setup_age_encrypted_file("dummy");

    let config = HashMap::from([
        ("identity".to_string(), toml::Value::String(identity_file.path().to_str().unwrap().to_string())),
    ]);
    let resolver = AgeResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("/nonexistent/file.age".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_age --features age-provider`
Expected: FAIL — `AgeResolver` not found.

- [ ] **Step 3: Implement AgeResolver**

```rust
// crates/secret-resolvers/src/age.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::Read;

pub struct AgeResolver {
    identity_path: String,
}

impl AgeResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let identity_path = config
            .get("identity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ConfigError("age provider requires 'identity' (path to key file)".into()))?
            .to_string();
        Ok(Self { identity_path })
    }
}

#[async_trait]
impl SecretResolver for AgeResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let file_path = request.get_str("ref")?;

        let identity_contents = std::fs::read_to_string(&self.identity_path)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to read identity file: {e}")))?;

        let identities: Vec<Box<dyn age::Identity>> = identity_contents
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .filter_map(|l| {
                l.parse::<age::x25519::Identity>()
                    .ok()
                    .map(|i| Box::new(i) as Box<dyn age::Identity>)
            })
            .collect();

        if identities.is_empty() {
            return Err(ResolverError::ConfigError("no valid age identities found in key file".into()));
        }

        let encrypted = std::fs::File::open(file_path)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to open encrypted file '{file_path}': {e}")))?;

        let decryptor = age::Decryptor::new(encrypted)
            .map_err(|e| ResolverError::ResolutionFailed(format!("age decryption init failed: {e}")))?;

        let mut decrypted = String::new();
        match decryptor {
            age::Decryptor::Recipients(d) => {
                d.decrypt(identities.iter().map(|i| i.as_ref()))
                    .map_err(|e| ResolverError::ResolutionFailed(format!("age decryption failed: {e}")))?
                    .read_to_string(&mut decrypted)
                    .map_err(|e| ResolverError::ResolutionFailed(format!("failed to read decrypted data: {e}")))?;
            }
            _ => return Err(ResolverError::ResolutionFailed("unexpected age decryptor type".into())),
        }

        Ok(ResolvedSecret { value: decrypted, ttl: None })
    }
}
```

Update `lib.rs` to add `pub use age::AgeResolver;` under the `#[cfg(feature = "age-provider")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_age --features age-provider`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/age.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/
git commit -m "feat: add age encrypted file provider with integration tests"
```

---

## Task 6: AWS Provider + Integration Test (LocalStack)

**Files:**
- Create: `crates/secret-resolvers/src/aws.rs`
- Create: `crates/secret-resolvers/tests/integration_aws.rs`
- Create: `docker-compose.test.yml`

- [ ] **Step 1: Create docker-compose.test.yml**

```yaml
services:
  localstack:
    image: localstack/localstack
    ports:
      - "4566:4566"
    environment:
      - SERVICES=secretsmanager,ssm
      - DEFAULT_REGION=us-east-1
  vault:
    image: hashicorp/vault
    ports:
      - "8200:8200"
    environment:
      - VAULT_DEV_ROOT_TOKEN_ID=test-root-token
    command: server -dev -dev-listen-address=0.0.0.0:8200
    cap_add:
      - IPC_LOCK
```

- [ ] **Step 2: Write integration test**

Tests run against LocalStack. They seed a secret, then resolve it.

```rust
// crates/secret-resolvers/tests/integration_aws.rs
use secret_resolvers::{AwsResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

/// These tests require LocalStack running on localhost:4566.
/// Run: docker compose -f docker-compose.test.yml up -d localstack

async fn localstack_available() -> bool {
    reqwest::get("http://localhost:4566/_localstack/health")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn seed_secrets_manager_secret(name: &str, value: &str) {
    let config = aws_config::from_env()
        .endpoint_url("http://localhost:4566")
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;
    let client = aws_sdk_secretsmanager::Client::new(&config);
    // Delete if exists (ignore errors)
    let _ = client.delete_secret().secret_id(name).force_delete_without_recovery(true).send().await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    client
        .create_secret()
        .name(name)
        .secret_string(value)
        .send()
        .await
        .unwrap();
}

async fn seed_ssm_parameter(name: &str, value: &str) {
    let config = aws_config::from_env()
        .endpoint_url("http://localhost:4566")
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;
    let client = aws_sdk_ssm::Client::new(&config);
    client
        .put_parameter()
        .name(name)
        .value(value)
        .r#type(aws_sdk_ssm::types::ParameterType::SecureString)
        .overwrite(true)
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_aws_sm_resolves_secret() {
    if !localstack_available().await {
        eprintln!("Skipping: LocalStack not available");
        return;
    }

    seed_secrets_manager_secret("dotvault-test/api-key", "sk-test-aws-123").await;

    let config = HashMap::from([
        ("region".to_string(), toml::Value::String("us-east-1".to_string())),
        ("endpoint_url".to_string(), toml::Value::String("http://localhost:4566".to_string())),
    ]);
    let resolver = AwsResolver::new(config).await.unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("sm://dotvault-test/api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "sk-test-aws-123");
}

#[tokio::test]
async fn test_aws_sm_resolves_json_field() {
    if !localstack_available().await {
        eprintln!("Skipping: LocalStack not available");
        return;
    }

    seed_secrets_manager_secret("dotvault-test/multi", r#"{"user":"admin","pass":"secret"}"#).await;

    let config = HashMap::from([
        ("region".to_string(), toml::Value::String("us-east-1".to_string())),
        ("endpoint_url".to_string(), toml::Value::String("http://localhost:4566".to_string())),
    ]);
    let resolver = AwsResolver::new(config).await.unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("sm://dotvault-test/multi".to_string())),
            ("field".to_string(), toml::Value::String("pass".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "secret");
}

#[tokio::test]
async fn test_aws_ssm_resolves_parameter() {
    if !localstack_available().await {
        eprintln!("Skipping: LocalStack not available");
        return;
    }

    seed_ssm_parameter("/dotvault/test/db-url", "postgres://localhost/test").await;

    let config = HashMap::from([
        ("region".to_string(), toml::Value::String("us-east-1".to_string())),
        ("endpoint_url".to_string(), toml::Value::String("http://localhost:4566".to_string())),
    ]);
    let resolver = AwsResolver::new(config).await.unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("ssm:///dotvault/test/db-url".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "postgres://localhost/test");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `docker compose -f docker-compose.test.yml up -d localstack && cargo test -p secret-resolvers --test integration_aws --features aws`
Expected: FAIL — `AwsResolver` not found.

- [ ] **Step 4: Implement AwsResolver**

```rust
// crates/secret-resolvers/src/aws.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct AwsResolver {
    sm_client: aws_sdk_secretsmanager::Client,
    ssm_client: aws_sdk_ssm::Client,
}

impl AwsResolver {
    pub async fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let mut builder = aws_config::from_env();
        if let Some(region) = config.get("region").and_then(|v| v.as_str()) {
            builder = builder.region(aws_config::Region::new(region.to_string()));
        }
        if let Some(profile) = config.get("profile").and_then(|v| v.as_str()) {
            builder = builder.profile_name(profile);
        }
        if let Some(endpoint) = config.get("endpoint_url").and_then(|v| v.as_str()) {
            builder = builder.endpoint_url(endpoint);
        }
        let aws_config = builder.load().await;
        Ok(Self {
            sm_client: aws_sdk_secretsmanager::Client::new(&aws_config),
            ssm_client: aws_sdk_ssm::Client::new(&aws_config),
        })
    }
}

#[async_trait]
impl SecretResolver for AwsResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let reference = request.get_str("ref")?;

        if let Some(secret_name) = reference.strip_prefix("sm://") {
            self.resolve_secrets_manager(secret_name, request.get_str_opt("field")).await
        } else if let Some(param_name) = reference.strip_prefix("ssm://") {
            self.resolve_ssm(param_name).await
        } else {
            Err(ResolverError::ResolutionFailed(
                format!("AWS ref must start with 'sm://' or 'ssm://': got '{reference}'"),
            ))
        }
    }
}

impl AwsResolver {
    async fn resolve_secrets_manager(&self, secret_name: &str, field: Option<&str>) -> Result<ResolvedSecret> {
        let response = self
            .sm_client
            .get_secret_value()
            .secret_id(secret_name)
            .send()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("AWS Secrets Manager: {e}")))?;

        let secret_string = response
            .secret_string()
            .ok_or_else(|| ResolverError::ResolutionFailed("no string value in secret".into()))?;

        let value = if let Some(field) = field {
            let parsed: serde_json::Value = serde_json::from_str(secret_string)
                .map_err(|e| ResolverError::ResolutionFailed(format!("JSON parse: {e}")))?;
            parsed
                .get(field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ResolverError::ResolutionFailed(format!("field '{field}' not found")))?
                .to_string()
        } else {
            secret_string.to_string()
        };

        Ok(ResolvedSecret { value, ttl: None })
    }

    async fn resolve_ssm(&self, param_name: &str) -> Result<ResolvedSecret> {
        let response = self
            .ssm_client
            .get_parameter()
            .name(param_name)
            .with_decryption(true)
            .send()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("AWS SSM: {e}")))?;

        let value = response
            .parameter()
            .and_then(|p| p.value())
            .ok_or_else(|| ResolverError::ResolutionFailed("no value in SSM parameter".into()))?
            .to_string();

        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Update `lib.rs` to add `pub use aws::AwsResolver;` under the `#[cfg(feature = "aws")]` block.

Also add `reqwest` to `[dev-dependencies]` in `crates/secret-resolvers/Cargo.toml` for the LocalStack health check, and add `aws-config`, `aws-sdk-secretsmanager`, `aws-sdk-ssm`, `tokio` to dev-dependencies for the test seeding:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full"] }
tempfile = "3"
reqwest = { version = "0.12" }
aws-config = "1"
aws-sdk-secretsmanager = "1"
aws-sdk-ssm = "1"
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_aws --features aws`
Expected: 3 tests pass (or skip if LocalStack is not running).

- [ ] **Step 6: Commit**

```bash
git add crates/secret-resolvers/src/aws.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/ crates/secret-resolvers/Cargo.toml docker-compose.test.yml
git commit -m "feat: add AWS provider (Secrets Manager + SSM) with LocalStack integration tests"
```

---

## Task 7: HashiCorp Vault Provider + Integration Test

**Files:**
- Create: `crates/secret-resolvers/src/hashicorp.rs`
- Create: `crates/secret-resolvers/tests/integration_hashicorp.rs`

- [ ] **Step 1: Write integration test**

Tests run against Vault dev server from docker-compose.test.yml (token: `test-root-token`).

```rust
// crates/secret-resolvers/tests/integration_hashicorp.rs
use secret_resolvers::{HashiCorpResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

async fn vault_available() -> bool {
    reqwest::get("http://localhost:8200/v1/sys/health")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn seed_vault_secret(path: &str, data: serde_json::Value) {
    let client = reqwest::Client::new();
    client
        .post(format!("http://localhost:8200/v1/secret/data/{path}"))
        .header("X-Vault-Token", "test-root-token")
        .json(&serde_json::json!({ "data": data }))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_hashicorp_resolves_kv_secret() {
    if !vault_available().await {
        eprintln!("Skipping: Vault not available");
        return;
    }

    seed_vault_secret("dotvault/test", serde_json::json!({
        "api_key": "vault-secret-123"
    }))
    .await;

    let config = HashMap::from([
        ("address".to_string(), toml::Value::String("http://localhost:8200".to_string())),
        ("token".to_string(), toml::Value::String("test-root-token".to_string())),
    ]);
    let resolver = HashiCorpResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("secret/data/dotvault/test".to_string())),
            ("field".to_string(), toml::Value::String("api_key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "vault-secret-123");
}

#[tokio::test]
async fn test_hashicorp_errors_on_missing_field() {
    if !vault_available().await {
        eprintln!("Skipping: Vault not available");
        return;
    }

    seed_vault_secret("dotvault/test2", serde_json::json!({
        "existing": "value"
    }))
    .await;

    let config = HashMap::from([
        ("address".to_string(), toml::Value::String("http://localhost:8200".to_string())),
        ("token".to_string(), toml::Value::String("test-root-token".to_string())),
    ]);
    let resolver = HashiCorpResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("secret/data/dotvault/test2".to_string())),
            ("field".to_string(), toml::Value::String("nonexistent".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose -f docker-compose.test.yml up -d vault && cargo test -p secret-resolvers --test integration_hashicorp --features hashicorp`
Expected: FAIL — `HashiCorpResolver` not found.

- [ ] **Step 3: Implement HashiCorpResolver**

```rust
// crates/secret-resolvers/src/hashicorp.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct HashiCorpResolver {
    client: reqwest::Client,
    address: String,
    token: Option<String>,
    namespace: Option<String>,
}

impl HashiCorpResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        // Try config, then VAULT_ADDR env var
        let address = config
            .get("address")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_ADDR").ok())
            .unwrap_or_else(|| "http://127.0.0.1:8200".to_string())
            .trim_end_matches('/')
            .to_string();

        let token = config
            .get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_TOKEN").ok());

        let namespace = config
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_NAMESPACE").ok());

        Ok(Self {
            client: reqwest::Client::new(),
            address,
            token,
            namespace,
        })
    }
}

#[async_trait]
impl SecretResolver for HashiCorpResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let path = request.get_str("ref")?;
        let field = request.get_str("field")?;

        let url = format!("{}/v1/{}", self.address, path);
        let mut req = self.client.get(&url);
        if let Some(token) = &self.token {
            req = req.header("X-Vault-Token", token);
        }
        if let Some(namespace) = &self.namespace {
            req = req.header("X-Vault-Namespace", namespace);
        }

        let response = req.send().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("Vault request failed: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(ResolverError::ResolutionFailed(format!(
                "Vault returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("Vault response parse error: {e}"))
        })?;

        // KV v2: { "data": { "data": { ...fields... } } }
        let value = body
            .pointer("/data/data")
            .and_then(|d| d.get(field))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ResolverError::ResolutionFailed(format!("field '{field}' not found in Vault response"))
            })?
            .to_string();

        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Note: Uses `reqwest` directly instead of `vaultrs` — simpler, fewer dependencies, and we only need KV v2 read. The Cargo.toml in Task 1 already has `hashicorp = ["dep:reqwest"]`.

Update `lib.rs` to add `pub use hashicorp::HashiCorpResolver;` under the `#[cfg(feature = "hashicorp")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_hashicorp --features hashicorp`
Expected: 2 tests pass (or skip if Vault is not running).

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/hashicorp.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/ crates/secret-resolvers/Cargo.toml
git commit -m "feat: add HashiCorp Vault provider with integration tests"
```

---

## Task 8: GCP Provider + Integration Test (wiremock)

**Files:**
- Create: `crates/secret-resolvers/src/gcp.rs`
- Create: `crates/secret-resolvers/tests/integration_gcp.rs`

- [ ] **Step 1: Write integration test**

Uses `wiremock` to fake the GCP Secret Manager REST API. The API surface is one endpoint: `GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access`.

```rust
// crates/secret-resolvers/tests/integration_gcp.rs
use secret_resolvers::{GcpResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};
use base64::Engine;

#[tokio::test]
async fn test_gcp_resolves_secret() {
    let mock_server = MockServer::start().await;

    let secret_value = base64::engine::general_purpose::STANDARD.encode("gcp-secret-123");
    let response_body = serde_json::json!({
        "payload": {
            "data": secret_value
        }
    });

    Mock::given(method("GET"))
        .and(path("/v1/projects/my-project/secrets/api-key/versions/latest:access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("project".to_string(), toml::Value::String("my-project".to_string())),
        ("endpoint_url".to_string(), toml::Value::String(mock_server.uri())),
    ]);
    let resolver = GcpResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "gcp-secret-123");
}

#[tokio::test]
async fn test_gcp_resolves_full_resource_name() {
    let mock_server = MockServer::start().await;

    let secret_value = base64::engine::general_purpose::STANDARD.encode("full-ref-value");
    let response_body = serde_json::json!({
        "payload": {
            "data": secret_value
        }
    });

    Mock::given(method("GET"))
        .and(path("/v1/projects/other-proj/secrets/db-pass/versions/2:access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("endpoint_url".to_string(), toml::Value::String(mock_server.uri())),
    ]);
    let resolver = GcpResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("projects/other-proj/secrets/db-pass/versions/2".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "full-ref-value");
}
```

Add to `[dev-dependencies]` in `crates/secret-resolvers/Cargo.toml`:

```toml
wiremock = "0.6"
base64 = "0.22"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_gcp --features gcp`
Expected: FAIL — `GcpResolver` not found.

- [ ] **Step 3: Implement GcpResolver**

```rust
// crates/secret-resolvers/src/gcp.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct GcpResolver {
    client: reqwest::Client,
    project: Option<String>,
    endpoint_url: String,
}

impl GcpResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let project = config.get("project").and_then(|v| v.as_str()).map(String::from);
        let endpoint_url = config
            .get("endpoint_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://secretmanager.googleapis.com")
            .trim_end_matches('/')
            .to_string();

        Ok(Self {
            client: reqwest::Client::new(),
            project,
            endpoint_url,
        })
    }
}

#[async_trait]
impl SecretResolver for GcpResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let reference = request.get_str("ref")?;
        let version = request.get_str_opt("version").unwrap_or("latest");

        // Support both short name ("api-key") and full resource name ("projects/X/secrets/Y/versions/Z")
        let resource_path = if reference.starts_with("projects/") {
            format!("{reference}:access")
        } else {
            let project = self.project.as_deref().ok_or_else(|| {
                ResolverError::ConfigError("GCP provider requires 'project' config for short secret names".into())
            })?;
            format!("projects/{project}/secrets/{reference}/versions/{version}:access")
        };

        let url = format!("{}/v1/{}", self.endpoint_url, resource_path);

        let mut req = self.client.get(&url);
        // In production, add Authorization header from Application Default Credentials.
        // For now, attempt to use gcloud CLI token if available.
        if let Ok(token) = std::env::var("GOOGLE_ACCESS_TOKEN") {
            req = req.bearer_auth(&token);
        }

        let response = req.send().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("GCP Secret Manager request failed: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(ResolverError::ResolutionFailed(format!(
                "GCP Secret Manager returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("GCP response parse error: {e}"))
        })?;

        let encoded = body
            .pointer("/payload/data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ResolutionFailed("no payload data in GCP response".into()))?;

        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| ResolverError::ResolutionFailed(format!("base64 decode: {e}")))?;

        let value = String::from_utf8(decoded)
            .map_err(|e| ResolverError::ResolutionFailed(format!("UTF-8 decode: {e}")))?;

        Ok(ResolvedSecret { value, ttl: None })
    }
}
```

Add `base64 = "0.22"` to `[dependencies]` in `crates/secret-resolvers/Cargo.toml` (needed at runtime for GCP, behind the gcp feature):

```toml
base64 = { version = "0.22", optional = true }
```

And update the gcp feature:
```toml
gcp = ["dep:reqwest", "dep:base64"]
```

Update `lib.rs` to add `pub use gcp::GcpResolver;` under the `#[cfg(feature = "gcp")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_gcp --features gcp`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/gcp.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/ crates/secret-resolvers/Cargo.toml
git commit -m "feat: add GCP Secret Manager provider with wiremock integration tests"
```

---

## Task 9: keyzero Provider + Integration Test (wiremock)

**Files:**
- Create: `crates/secret-resolvers/src/keyzero.rs`
- Create: `crates/secret-resolvers/tests/integration_keyzero.rs`

- [ ] **Step 1: Write integration test**

```rust
// crates/secret-resolvers/tests/integration_keyzero.rs
use secret_resolvers::{KeyzeroResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path, body_json};

#[tokio::test]
async fn test_keyzero_resolves_secret() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "allowed": true,
        "resource": "db/password",
        "policy": "allow-dev",
        "secrets": {
            "password": {
                "mode": "direct",
                "value": "keyzero-resolved-123",
                "ttl": 300
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("endpoint".to_string(), toml::Value::String(mock_server.uri())),
        ("token".to_string(), toml::Value::String("test-jwt-token".to_string())),
    ]);
    let resolver = KeyzeroResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("db/password".to_string())),
            ("secret_name".to_string(), toml::Value::String("password".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "keyzero-resolved-123");
    assert_eq!(result.ttl, Some(300));
}

#[tokio::test]
async fn test_keyzero_errors_on_denied() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "allowed": false,
        "resource": "db/password",
        "policy": "default-deny",
        "reason": "no matching policy"
    });

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("endpoint".to_string(), toml::Value::String(mock_server.uri())),
        ("token".to_string(), toml::Value::String("test-jwt-token".to_string())),
    ]);
    let resolver = KeyzeroResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("db/password".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secret-resolvers --test integration_keyzero --features keyzero`
Expected: FAIL — `KeyzeroResolver` not found.

- [ ] **Step 3: Implement KeyzeroResolver**

```rust
// crates/secret-resolvers/src/keyzero.rs
use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct KeyzeroResolver {
    client: reqwest::Client,
    endpoint: String,
    token: Option<String>,
}

impl KeyzeroResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let endpoint = config
            .get("endpoint")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ConfigError("keyzero provider requires 'endpoint'".into()))?
            .trim_end_matches('/')
            .to_string();
        let token = config.get("token").and_then(|v| v.as_str()).map(String::from);

        Ok(Self {
            client: reqwest::Client::new(),
            endpoint,
            token,
        })
    }
}

#[async_trait]
impl SecretResolver for KeyzeroResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let resource_id = request.get_str("ref")?;
        let secret_name = request.get_str_opt("secret_name");

        let url = format!("{}/v1/resolve", self.endpoint);
        let mut req = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "resource": resource_id }));

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("keyzero request failed: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(ResolverError::ResolutionFailed(format!(
                "keyzero returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("keyzero response parse error: {e}"))
        })?;

        let allowed = body.get("allowed").and_then(|v| v.as_bool()).unwrap_or(false);
        if !allowed {
            let reason = body.get("reason").and_then(|v| v.as_str()).unwrap_or("access denied");
            return Err(ResolverError::ResolutionFailed(format!("keyzero: {reason}")));
        }

        let secrets = body.get("secrets").and_then(|v| v.as_object()).ok_or_else(|| {
            ResolverError::ResolutionFailed("keyzero response missing 'secrets'".into())
        })?;

        // If secret_name is specified, get that specific secret.
        // Otherwise, get the first (or only) secret.
        let entry = if let Some(name) = secret_name {
            secrets.get(name).ok_or_else(|| {
                ResolverError::ResolutionFailed(format!("secret '{name}' not found in keyzero response"))
            })?
        } else {
            secrets.values().next().ok_or_else(|| {
                ResolverError::ResolutionFailed("no secrets in keyzero response".into())
            })?
        };

        let value = entry
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ResolutionFailed("secret entry missing 'value'".into()))?
            .to_string();

        let ttl = entry.get("ttl").and_then(|v| v.as_u64());

        Ok(ResolvedSecret { value, ttl })
    }
}
```

Update `lib.rs` to add `pub use keyzero::KeyzeroResolver;` under the `#[cfg(feature = "keyzero")]` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p secret-resolvers --test integration_keyzero --features keyzero`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/secret-resolvers/src/keyzero.rs crates/secret-resolvers/src/lib.rs crates/secret-resolvers/tests/
git commit -m "feat: add keyzero provider with wiremock integration tests"
```

---

## Task 10: dotvault Config Parsing

**Files:**
- Create: `crates/dotvault/src/config.rs`
- Modify: `crates/dotvault/src/main.rs`

- [ ] **Step 1: Write config parsing tests (inline unit tests)**

```rust
// crates/dotvault/src/config.rs
// ... (implementation + tests below)

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_basic_config() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, r#"
[secrets]
OPENAI_API_KEY = {{ provider = "env", ref = "MY_KEY" }}
DB_URL = {{ provider = "1password", ref = "op://Vault/Item/field" }}
"#).unwrap();

        let config = DotVaultConfig::load(f.path()).unwrap();
        assert_eq!(config.secrets.len(), 2);
        assert_eq!(config.secrets["OPENAI_API_KEY"].provider, "env");
        assert_eq!(config.secrets["DB_URL"].provider, "1password");
    }

    #[test]
    fn test_parse_with_provider_config() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, r#"
[providers.1password]
account = "my-team.1password.com"

[secrets]
KEY = {{ provider = "1password", ref = "op://Vault/Item/field" }}
"#).unwrap();

        let config = DotVaultConfig::load(f.path()).unwrap();
        let provider_config = config.providers.get("1password").unwrap();
        assert_eq!(provider_config.get("account").unwrap().as_str().unwrap(), "my-team.1password.com");
    }

    #[test]
    fn test_local_replaces_shared() {
        let dir = tempfile::tempdir().unwrap();

        let shared_path = dir.path().join(".dotvault.toml");
        std::fs::write(&shared_path, r#"
[secrets]
KEY = { provider = "env", ref = "SHARED_KEY" }
"#).unwrap();

        let local_path = dir.path().join(".dotvault.local.toml");
        std::fs::write(&local_path, r#"
[secrets]
KEY = { provider = "env", ref = "LOCAL_KEY" }
"#).unwrap();

        let config = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        assert_eq!(config.secrets["KEY"].extra.get("ref").unwrap().as_str().unwrap(), "LOCAL_KEY");
    }

    #[test]
    fn test_error_on_no_config() {
        let dir = tempfile::tempdir().unwrap();
        let result = DotVaultConfig::load_from_dir(dir.path());
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dotvault`
Expected: FAIL — `DotVaultConfig` not found.

- [ ] **Step 3: Implement config.rs**

```rust
// crates/dotvault/src/config.rs
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct DotVaultConfig {
    #[serde(default)]
    pub providers: HashMap<String, HashMap<String, toml::Value>>,
    pub secrets: HashMap<String, SecretEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SecretEntry {
    pub provider: String,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

impl DotVaultConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(config)
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let local = dir.join(".dotvault.local.toml");
        if local.exists() {
            return Self::load(&local);
        }
        let shared = dir.join(".dotvault.toml");
        if shared.exists() {
            return Self::load(&shared);
        }
        bail!("No .dotvault.toml or .dotvault.local.toml found in {}", dir.display());
    }

    /// Merge provider config from global config file (~/.config/dotvault/config.toml).
    /// Project-level config takes precedence.
    pub fn merge_global_providers(&mut self) -> Result<()> {
        let global_path = dirs::config_dir()
            .map(|d| d.join("dotvault").join("config.toml"));

        if let Some(path) = global_path {
            if path.exists() {
                let global: GlobalConfig = toml::from_str(
                    &std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?,
                )
                .with_context(|| format!("failed to parse {}", path.display()))?;

                for (name, global_provider_config) in global.providers {
                    let project_config = self.providers.entry(name).or_default();
                    // Global values are inserted only if not already set at project level
                    for (key, value) in global_provider_config {
                        project_config.entry(key).or_insert(value);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GlobalConfig {
    #[serde(default)]
    providers: HashMap<String, HashMap<String, toml::Value>>,
}

#[cfg(test)]
mod tests {
    // ... tests from Step 1 go here
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `crates/dotvault/Cargo.toml`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dotvault`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/dotvault/src/config.rs crates/dotvault/Cargo.toml
git commit -m "feat: add config parsing with local-replaces-shared and global provider config"
```

---

## Task 11: Resolver Orchestration

**Files:**
- Create: `crates/dotvault/src/resolve.rs`

- [ ] **Step 1: Write test for resolver orchestration**

```rust
// crates/dotvault/src/resolve.rs (inline tests at bottom)
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_all_with_env_provider() {
        std::env::set_var("DOTVAULT_RESOLVE_TEST", "resolved-value");

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets: HashMap::from([(
                "MY_SECRET".to_string(),
                SecretEntry {
                    provider: "env".to_string(),
                    extra: HashMap::from([
                        ("ref".to_string(), toml::Value::String("DOTVAULT_RESOLVE_TEST".to_string())),
                    ]),
                },
            )]),
        };

        let result = resolve_all(&config).await.unwrap();
        assert_eq!(result.get("MY_SECRET").unwrap(), "resolved-value");
    }

    #[tokio::test]
    async fn test_resolve_all_fails_on_unknown_provider() {
        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets: HashMap::from([(
                "MY_SECRET".to_string(),
                SecretEntry {
                    provider: "nonexistent".to_string(),
                    extra: HashMap::from([
                        ("ref".to_string(), toml::Value::String("anything".to_string())),
                    ]),
                },
            )]),
        };

        let result = resolve_all(&config).await;
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dotvault resolve`
Expected: FAIL — `resolve_all` not found.

- [ ] **Step 3: Implement resolve.rs**

```rust
// crates/dotvault/src/resolve.rs
use crate::config::{DotVaultConfig, SecretEntry};
use anyhow::{bail, Context, Result};
use secret_resolvers::{ResolveRequest, SecretResolver};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn resolve_all(config: &DotVaultConfig) -> Result<HashMap<String, String>> {
    let providers = build_providers(config)?;

    let futures: Vec<_> = config
        .secrets
        .iter()
        .map(|(env_name, entry)| {
            let provider = providers.get(&entry.provider).cloned();
            let env_name = env_name.clone();
            let params = entry.extra.clone();
            async move {
                let provider = provider.ok_or_else(|| {
                    anyhow::anyhow!("Unknown provider '{}' for secret {}", entry.provider, env_name)
                })?;
                let request = ResolveRequest { params };
                let resolved = provider
                    .resolve(&request)
                    .await
                    .with_context(|| format!("Error resolving {} (provider: {})", env_name, entry.provider))?;
                Ok::<_, anyhow::Error>((env_name, resolved.value))
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    let mut errors = Vec::new();
    let mut secrets = HashMap::new();
    for result in results {
        match result {
            Ok((name, value)) => { secrets.insert(name, value); }
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        let msg = errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        bail!("Failed to resolve secrets:\n{msg}");
    }

    Ok(secrets)
}

fn build_providers(
    config: &DotVaultConfig,
) -> Result<HashMap<String, Arc<dyn SecretResolver>>> {
    let mut providers: HashMap<String, Arc<dyn SecretResolver>> = HashMap::new();

    // Collect unique provider names
    let provider_names: std::collections::HashSet<_> =
        config.secrets.values().map(|e| e.provider.as_str()).collect();

    for name in provider_names {
        let provider_config = config.providers.get(name).cloned().unwrap_or_default();
        let resolver: Arc<dyn SecretResolver> = create_provider(name, provider_config)?;
        providers.insert(name.to_string(), resolver);
    }

    Ok(providers)
}

fn create_provider(
    name: &str,
    config: HashMap<String, toml::Value>,
) -> Result<Arc<dyn SecretResolver>> {
    match name {
        "env" => Ok(Arc::new(secret_resolvers::EnvResolver::new(config)?)),
        #[cfg(feature = "onepassword")]
        "1password" => Ok(Arc::new(secret_resolvers::OnePasswordResolver::new(config)?)),
        #[cfg(feature = "keychain")]
        "keychain" => Ok(Arc::new(secret_resolvers::KeychainResolver::new(config)?)),
        #[cfg(feature = "age-provider")]
        "age" => Ok(Arc::new(secret_resolvers::AgeResolver::new(config)?)),
        #[cfg(feature = "hashicorp")]
        "hashicorp" => Ok(Arc::new(secret_resolvers::HashiCorpResolver::new(config)?)),
        #[cfg(feature = "gcp")]
        "gcp" => Ok(Arc::new(secret_resolvers::GcpResolver::new(config)?)),
        // Note: aws and keyzero have async constructors, handled separately
        other => bail!("Unknown provider '{other}'"),
    }
}

#[cfg(test)]
mod tests {
    // ... tests from Step 1
}
```

Note: AWS and keyzero providers have async constructors. Update `create_provider` to be async or handle them as special cases. For simplicity, make `build_providers` async:

```rust
async fn build_providers(...) -> Result<...> { ... }

async fn create_provider(...) -> Result<Arc<dyn SecretResolver>> {
    match name {
        // ... sync providers ...
        #[cfg(feature = "aws")]
        "aws" => Ok(Arc::new(secret_resolvers::AwsResolver::new(config).await?)),
        #[cfg(feature = "keyzero")]
        "keyzero" => Ok(Arc::new(secret_resolvers::KeyzeroResolver::new(config)?)),
        other => bail!("Unknown provider '{other}'"),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dotvault resolve`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/dotvault/src/resolve.rs
git commit -m "feat: add concurrent secret resolution with provider registry"
```

---

## Task 12: CLI Commands (run, export, init, hook)

**Files:**
- Create: `crates/dotvault/src/run.rs`
- Create: `crates/dotvault/src/export.rs`
- Modify: `crates/dotvault/src/main.rs`

- [ ] **Step 1: Implement main.rs with clap CLI**

```rust
// crates/dotvault/src/main.rs
mod config;
mod export;
mod resolve;
mod run;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dotvault", about = "Resolve secrets from pluggable backends into your dev environment")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Resolve secrets and run a command with them as env vars
    Run {
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Resolve secrets and print export statements
    Export,
    /// Generate a starter .dotvault.toml
    Init,
    /// Print shell hook snippet
    Hook {
        #[arg(long)]
        shell: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { command } => run::exec(command).await,
        Commands::Export => export::exec().await,
        Commands::Init => init(),
        Commands::Hook { shell } => hook(&shell),
    }
}

fn init() -> anyhow::Result<()> {
    let path = std::path::Path::new(".dotvault.toml");
    if path.exists() {
        anyhow::bail!(".dotvault.toml already exists");
    }
    std::fs::write(
        path,
        r#"# dotvault configuration
# See: https://github.com/jondot/dotvault

[secrets]
# EXAMPLE_KEY = { provider = "env", ref = "MY_ENV_VAR" }
# API_KEY = { provider = "1password", ref = "op://Vault/Item/field" }
"#,
    )?;
    println!("Created .dotvault.toml");
    Ok(())
}

fn hook(shell: &str) -> anyhow::Result<()> {
    match shell {
        "zsh" => print!(
            r#"_dotvault_hook() {{
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}}
autoload -U add-zsh-hook
add-zsh-hook chpwd _dotvault_hook
_dotvault_hook
"#
        ),
        "bash" => print!(
            r#"_dotvault_hook() {{
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}}
PROMPT_COMMAND="_dotvault_hook;$PROMPT_COMMAND"
"#
        ),
        "fish" => print!(
            r#"function _dotvault_hook --on-variable PWD
  if test -f .dotvault.toml; or test -f .dotvault.local.toml
    eval (dotvault export)
  end
end
_dotvault_hook
"#
        ),
        other => anyhow::bail!("Unsupported shell: {other}. Use zsh, bash, or fish."),
    }
    Ok(())
}
```

- [ ] **Step 2: Implement run.rs**

```rust
// crates/dotvault/src/run.rs
use crate::config::DotVaultConfig;
use crate::resolve::resolve_all;
use anyhow::{bail, Result};
use std::os::unix::process::CommandExt;

pub async fn exec(command: Vec<String>) -> Result<()> {
    if command.is_empty() {
        bail!("Usage: dotvault run -- <command> [args...]");
    }

    let mut config = DotVaultConfig::load_from_dir(&std::env::current_dir()?)?;
    config.merge_global_providers()?;
    let secrets = resolve_all(&config).await?;

    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..]);
    for (key, value) in &secrets {
        cmd.env(key, value);
    }

    // exec replaces the current process
    let err = cmd.exec();
    bail!("Failed to exec '{}': {err}", command[0]);
}
```

- [ ] **Step 3: Implement export.rs**

```rust
// crates/dotvault/src/export.rs
use crate::config::DotVaultConfig;
use crate::resolve::resolve_all;
use anyhow::Result;

pub async fn exec() -> Result<()> {
    let mut config = DotVaultConfig::load_from_dir(&std::env::current_dir()?)?;
    config.merge_global_providers()?;
    let secrets = resolve_all(&config).await?;

    for (key, value) in &secrets {
        // Escape single quotes in value for safe shell export
        let escaped = value.replace('\'', "'\\''");
        println!("export {key}='{escaped}'");
    }

    Ok(())
}
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p dotvault`
Expected: compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/dotvault/src/
git commit -m "feat: add CLI commands (run, export, init, hook)"
```

---

## Task 13: E2E Tests

**Files:**
- Create: `crates/dotvault/tests/e2e_run.rs`
- Create: `crates/dotvault/tests/e2e_export.rs`
- Create: `crates/dotvault/tests/e2e_init.rs`
- Create: `crates/dotvault/tests/e2e_errors.rs`

Add to `crates/dotvault/Cargo.toml`:
```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 1: Write E2E tests**

```rust
// crates/dotvault/tests/e2e_export.rs
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

#[test]
fn test_export_with_env_provider() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".dotvault.toml");
    std::fs::write(&config_path, r#"
[secrets]
MY_SECRET = { provider = "env", ref = "DOTVAULT_E2E_TEST" }
"#).unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .env("DOTVAULT_E2E_TEST", "e2e-value")
        .args(["export"])
        .assert()
        .success()
        .stdout(predicate::str::contains("export MY_SECRET='e2e-value'"));
}

#[test]
fn test_export_uses_local_over_shared() {
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(dir.path().join(".dotvault.toml"), r#"
[secrets]
KEY = { provider = "env", ref = "SHARED_VAR" }
"#).unwrap();

    std::fs::write(dir.path().join(".dotvault.local.toml"), r#"
[secrets]
KEY = { provider = "env", ref = "LOCAL_VAR" }
"#).unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .env("LOCAL_VAR", "local-value")
        .args(["export"])
        .assert()
        .success()
        .stdout(predicate::str::contains("export KEY='local-value'"));
}
```

```rust
// crates/dotvault/tests/e2e_run.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_run_injects_env_vars() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".dotvault.toml"), r#"
[secrets]
MY_INJECTED = { provider = "env", ref = "DOTVAULT_E2E_RUN" }
"#).unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .env("DOTVAULT_E2E_RUN", "injected-value")
        .args(["run", "--", "env"])
        .assert()
        .success()
        .stdout(predicate::str::contains("MY_INJECTED=injected-value"));
}
```

```rust
// crates/dotvault/tests/e2e_init.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_init_creates_config() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .args(["init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .dotvault.toml"));

    assert!(dir.path().join(".dotvault.toml").exists());
}

#[test]
fn test_init_errors_if_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".dotvault.toml"), "").unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .args(["init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
```

```rust
// crates/dotvault/tests/e2e_errors.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_error_no_config_file() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .args(["export"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No .dotvault.toml"));
}

#[test]
fn test_error_unknown_provider() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".dotvault.toml"), r#"
[secrets]
KEY = { provider = "nonexistent", ref = "something" }
"#).unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .args(["export"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown provider"));
}

#[test]
fn test_error_missing_env_var() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".dotvault.toml"), r#"
[secrets]
KEY = { provider = "env", ref = "DOTVAULT_DEFINITELY_NOT_SET_12345" }
"#).unwrap();

    Command::cargo_bin("dotvault")
        .unwrap()
        .current_dir(dir.path())
        .env_remove("DOTVAULT_DEFINITELY_NOT_SET_12345")
        .args(["export"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error resolving KEY"));
}
```

- [ ] **Step 2: Run E2E tests**

Run: `cargo test -p dotvault --test e2e_export --test e2e_run --test e2e_init --test e2e_errors`
Expected: All tests pass.

- [ ] **Step 3: Fix any failing tests, iterate**

- [ ] **Step 4: Commit**

```bash
git add crates/dotvault/tests/ crates/dotvault/Cargo.toml
git commit -m "feat: add E2E tests for all CLI commands and error cases"
```

---

## Task 14: Shell Hooks

**Files:**
- Create: `shell/hook.zsh`
- Create: `shell/hook.bash`
- Create: `shell/hook.fish`

- [ ] **Step 1: Create shell hook files**

`shell/hook.zsh`:
```zsh
_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}
autoload -U add-zsh-hook
add-zsh-hook chpwd _dotvault_hook
_dotvault_hook
```

`shell/hook.bash`:
```bash
_dotvault_hook() {
  if [[ -f .dotvault.toml ]] || [[ -f .dotvault.local.toml ]]; then
    eval "$(dotvault export)"
  fi
}
PROMPT_COMMAND="_dotvault_hook;$PROMPT_COMMAND"
```

`shell/hook.fish`:
```fish
function _dotvault_hook --on-variable PWD
  if test -f .dotvault.toml; or test -f .dotvault.local.toml
    eval (dotvault export)
  end
end
_dotvault_hook
```

- [ ] **Step 2: Commit**

```bash
git add shell/
git commit -m "feat: add shell hook scripts for zsh, bash, and fish"
```

---

## Task 15: npm Distribution (from tooly)

**Files:**
- Create: `npm/dotvault/package.json`
- Create: `npm/dotvault/bin/dotvault`
- Create: `npm/cli-darwin-arm64/package.json`
- Create: `npm/cli-linux-x64/package.json`
- Create: `npm/cli-linux-arm64/package.json`

- [ ] **Step 1: Create npm package files**

Adapted directly from `~/projects/tooly/npm/`.

`npm/dotvault/package.json`:
```json
{
  "name": "@jondot/dotvault",
  "version": "0.1.0",
  "description": "Resolve secrets from pluggable backends into your dev environment",
  "license": "MIT",
  "bin": {
    "dotvault": "bin/dotvault"
  },
  "optionalDependencies": {
    "@jondot/dotvault-cli-darwin-arm64": "0.1.0",
    "@jondot/dotvault-cli-linux-x64": "0.1.0",
    "@jondot/dotvault-cli-linux-arm64": "0.1.0"
  }
}
```

`npm/dotvault/bin/dotvault` — copy from `~/projects/tooly/npm/tooly/bin/tooly` and replace:
- `TOOLY_BINARY` → `DOTVAULT_BINARY`
- `@jondot/tooly-cli-` → `@jondot/dotvault-cli-`
- `tooly` → `dotvault` (binary name, error messages)

`npm/cli-darwin-arm64/package.json`:
```json
{
  "name": "@jondot/dotvault-cli-darwin-arm64",
  "version": "0.1.0",
  "description": "dotvault binary for macOS ARM64",
  "license": "MIT",
  "os": ["darwin"],
  "cpu": ["arm64"]
}
```

`npm/cli-linux-x64/package.json`:
```json
{
  "name": "@jondot/dotvault-cli-linux-x64",
  "version": "0.1.0",
  "description": "dotvault binary for Linux x64",
  "license": "MIT",
  "os": ["linux"],
  "cpu": ["x64"]
}
```

`npm/cli-linux-arm64/package.json`:
```json
{
  "name": "@jondot/dotvault-cli-linux-arm64",
  "version": "0.1.0",
  "description": "dotvault binary for Linux ARM64",
  "license": "MIT",
  "os": ["linux"],
  "cpu": ["arm64"]
}
```

- [ ] **Step 2: Commit**

```bash
git add npm/
git commit -m "feat: add npm distribution packages (adapted from tooly)"
```

---

## Task 16: GitHub Actions Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `scripts/bump-version.sh`

- [ ] **Step 1: Create release.yml**

Adapted from `~/projects/tooly/.github/workflows/release.yml`. Key changes: binary name `dotvault`, workspace build `cargo build --release -p dotvault --target ${{ matrix.target }}`, npm package names.

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - runner: macos-latest
            target: aarch64-apple-darwin
            npm_pkg: cli-darwin-arm64
            binary: dotvault
          - runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            npm_pkg: cli-linux-x64
            binary: dotvault
          - runner: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            npm_pkg: cli-linux-arm64
            binary: dotvault
            cross: true

    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross-compilation tools (Linux ARM64)
        if: matrix.cross
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu
          echo "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc" >> $GITHUB_ENV

      - name: Build
        run: cargo build --release -p dotvault --target ${{ matrix.target }}

      - name: Copy binary to npm package
        shell: bash
        run: cp target/${{ matrix.target }}/release/${{ matrix.binary }} npm/${{ matrix.npm_pkg }}/

      - name: Package release binary
        shell: bash
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ${{ matrix.binary }}-${{ matrix.target }}.tar.gz ${{ matrix.binary }}

      - name: Upload npm artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.npm_pkg }}
          path: npm/${{ matrix.npm_pkg }}/

      - name: Upload release binary
        uses: actions/upload-artifact@v4
        with:
          name: release-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/${{ matrix.binary }}-${{ matrix.target }}.tar.gz

  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          registry-url: https://registry.npmjs.org

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Prepare platform packages
        run: |
          for pkg in cli-darwin-arm64 cli-linux-x64 cli-linux-arm64; do
            cp -r artifacts/$pkg/* npm/$pkg/
          done

      - name: Publish platform packages
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: |
          for pkg in cli-darwin-arm64 cli-linux-x64 cli-linux-arm64; do
            npm publish ./npm/$pkg --access public
          done

      - name: Publish main package
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: npm publish ./npm/dotvault --access public

      - name: Download release binaries
        uses: actions/download-artifact@v4
        with:
          pattern: release-*
          path: release-binaries
          merge-multiple: true

      - name: Create GitHub Release
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh release create ${{ github.ref_name }} --generate-notes release-binaries/*
```

- [ ] **Step 2: Create bump-version.sh**

Adapted from `~/projects/tooly/scripts/bump-version.sh`. Key changes: workspace Cargo.toml path, npm package names, remove Claude plugin manifest updates.

```bash
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

CURRENT=$(grep '^version' "$REPO_ROOT/crates/dotvault/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

BUMP="${1:-}"

case "$BUMP" in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
  "")
    echo "Current version: $CURRENT"
    echo "Usage: $0 <major|minor|patch>"
    exit 1
    ;;
  *)
    echo "Unknown bump type: $BUMP (use major, minor, or patch)"
    exit 1
    ;;
esac

VERSION="$MAJOR.$MINOR.$PATCH"
echo "Bumping $CURRENT -> $VERSION"

# Update crate Cargo.tomls
for crate in dotvault secret-resolvers; do
  CARGOFILE="$REPO_ROOT/crates/$crate/Cargo.toml"
  if [ -f "$CARGOFILE" ]; then
    sed -i '' "s/^version = \"$CURRENT\"/version = \"$VERSION\"/" "$CARGOFILE"
    echo "  Updated crates/$crate/Cargo.toml"
  fi
done

# Update platform packages
for pkg in cli-darwin-arm64 cli-linux-x64 cli-linux-arm64; do
  PKGJSON="$REPO_ROOT/npm/$pkg/package.json"
  if [ -f "$PKGJSON" ]; then
    node -e "
      const fs = require('fs');
      const pkg = JSON.parse(fs.readFileSync('$PKGJSON', 'utf8'));
      pkg.version = '$VERSION';
      fs.writeFileSync('$PKGJSON', JSON.stringify(pkg, null, 2) + '\n');
    "
    echo "  Updated npm/$pkg"
  fi
done

# Update main package (version + optionalDependencies)
MAIN_PKG="$REPO_ROOT/npm/dotvault/package.json"
if [ -f "$MAIN_PKG" ]; then
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$MAIN_PKG', 'utf8'));
    pkg.version = '$VERSION';
    if (pkg.optionalDependencies) {
      for (const dep of Object.keys(pkg.optionalDependencies)) {
        pkg.optionalDependencies[dep] = '$VERSION';
      }
    }
    fs.writeFileSync('$MAIN_PKG', JSON.stringify(pkg, null, 2) + '\n');
  "
  echo "  Updated npm/dotvault (main package)"
fi

# Sync Cargo.lock
(cd "$REPO_ROOT" && cargo generate-lockfile 2>/dev/null) && echo "  Updated Cargo.lock" || true

echo "Done. All packages set to $VERSION"
```

- [ ] **Step 3: Make bump script executable and commit**

```bash
chmod +x scripts/bump-version.sh
git add .github/ scripts/
git commit -m "feat: add GitHub Actions release workflow and version bump script"
```

---

## Task 17: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All unit + E2E tests pass (integration tests that need Docker will skip gracefully).

- [ ] **Step 2: Run integration tests with Docker backends**

Run: `docker compose -f docker-compose.test.yml up -d && cargo test --workspace --features all`
Expected: All tests pass including AWS (LocalStack) and HashiCorp Vault integration tests.

- [ ] **Step 3: Build release binary and smoke test**

Run: `cargo build --release -p dotvault`

Create a test config and run:
```bash
echo '[secrets]
TEST = { provider = "env", ref = "HOME" }' > /tmp/test-dotvault/.dotvault.toml
cd /tmp/test-dotvault
dotvault export
dotvault run -- env | grep TEST
```
Expected: `export TEST='/Users/jondot'` and `TEST=/Users/jondot` in output.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: address issues from final verification"
```
