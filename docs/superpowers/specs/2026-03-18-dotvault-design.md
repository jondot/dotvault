# dotvault Design Spec

## Problem

Developers need secrets (API keys, tokens, connection strings) in local development. Every current approach has drawbacks:

- In code: checked into git
- In shell profile: visible, not portable
- In `.env.local`: clear text on disk, not checked in but still exposed
- Manual export: tedious, breaks on rotation

When a secret is rotated (e.g., VP R&D cancels and reissues a token), redistribution is manual and ad-hoc (Slack, email).

Solutions like 1Password CLI solve injection but create vendor lock-in.

## Solution

**dotvault** is an open-source Rust CLI that resolves secrets from pluggable backends and injects them into the developer's environment. Secrets never touch the filesystem. The developer experience is zero-friction: secrets appear automatically when you enter a project directory.

## Architecture: Thin Orchestrator

dotvault reads a project config file, calls provider backends to resolve secrets, and either injects them into a subprocess or prints shell exports. No daemon, no encrypted files on disk.

```
.dotvault.toml → dotvault → [providers] → env vars → subprocess
```

## Project Structure

```
dotvault/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── dotvault/                 # the CLI binary
│   │   ├── src/
│   │   │   ├── main.rs           # CLI entry (clap)
│   │   │   ├── config.rs         # parse .dotvault.toml / .dotvault.local.toml
│   │   │   ├── resolve.rs        # takes config, calls providers, returns HashMap<String, String>
│   │   │   ├── run.rs            # `dotvault run -- <cmd>` subcommand
│   │   │   └── export.rs         # `dotvault export` subcommand
│   │   └── Cargo.toml
│   └── secret-resolvers/         # shared provider library (used by dotvault AND keyzero)
│       ├── src/
│       │   ├── lib.rs
│       │   ├── provider.rs       # SecretResolver trait, ResolveRequest, ResolvedSecret
│       │   ├── env.rs            # env var provider
│       │   ├── onepassword.rs    # 1Password CLI provider
│       │   ├── keychain.rs       # OS keychain provider (keyring crate)
│       │   ├── age.rs            # age-encrypted file provider
│       │   ├── aws.rs            # AWS Secrets Manager + SSM Parameter Store
│       │   ├── hashicorp.rs      # HashiCorp Vault
│       │   ├── gcp.rs            # Google Cloud Secret Manager
│       │   └── keyzero.rs        # keyzero-as-provider (HTTP client to keyzero server)
│       └── Cargo.toml
├── shell/
│   ├── hook.zsh
│   ├── hook.bash
│   └── hook.fish
├── npm/
│   ├── dotvault/                 # umbrella npm package
│   │   ├── package.json
│   │   └── bin/dotvault          # Node.js wrapper (platform detection)
│   ├── cli-darwin-arm64/
│   │   └── package.json
│   ├── cli-linux-x64/
│   │   └── package.json
│   └── cli-linux-arm64/
│       └── package.json
├── .github/
│   └── workflows/
│       └── release.yml           # build matrix + npm publish + GitHub Release
├── scripts/
│   └── bump-version.sh           # coordinated version bump across Cargo.toml + package.jsons
├── docker-compose.test.yml       # integration test backends
└── tests/
    ├── integration/              # real backend tests
    └── e2e/                      # full CLI tests
```

## CLI Commands

- `dotvault run -- <cmd>` — resolve all secrets, inject as env vars, exec the subprocess
- `dotvault export` — resolve all secrets, print `export KEY="VALUE"` lines to stdout
- `dotvault init` — generate a starter `.dotvault.toml` in the current directory
- `dotvault hook --shell zsh|bash|fish` — print the shell hook snippet for the user to add to their profile

## Config Format

### `.dotvault.toml` (checked into repo)

```toml
[providers.1password]
# optional provider-specific config (opaque passthrough)
account = "my-team.1password.com"

[secrets]
OPENAI_API_KEY = { provider = "1password", ref = "op://Engineering/OpenAI/api-key" }
DATABASE_URL = { provider = "1password", ref = "op://Engineering/DevDB/connection-string" }
```

### `.dotvault.local.toml` (gitignored, completely replaces .dotvault.toml)

```toml
[secrets]
OPENAI_API_KEY = { provider = "env", ref = "MY_PERSONAL_OPENAI_KEY" }
DATABASE_URL = { provider = "1password", ref = "op://Engineering/DevDB/connection-string" }
DEBUG_TOKEN = { provider = "keychain", ref = "debug-token" }
```

**If `.dotvault.local.toml` exists, it is used instead of `.dotvault.toml`. No merge, no override logic — full replacement.**

### Per-user global config (`~/.config/dotvault/config.toml`)

For provider configuration that applies across all projects:

```toml
[providers.1password]
account = "my-team.1password.com"

[providers.hashicorp]
address = "https://vault.mycompany.com"
```

Project-level `[providers.*]` config takes precedence over global config for the same keys.

## The `secret-resolvers` Shared Crate

This crate is the core provider abstraction. It is designed to serve both dotvault (simple consumer) and keyzero (advanced consumer with policy/orchestration layers).

### Trait

```rust
#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret>;
}

pub struct ResolveRequest {
    pub params: HashMap<String, toml::Value>,
}

pub struct ResolvedSecret {
    pub value: String,
    pub ttl: Option<u64>,
}
```

- `params` is an opaque bag of key-value pairs. Each provider defines what keys it expects.
- `ttl` is optional — dotvault ignores it, keyzero uses it for caching/refresh.
- Constructors are per-provider (not part of the trait). Each provider's `new()` takes a `HashMap<String, toml::Value>` for backend-level config.
- Provider config names (e.g. `1password` in TOML) are mapped to Rust module names (e.g. `onepassword`) in the provider registry. The registry handles this translation.

### Provider Catalog

| Provider | Crate/Tool | Backend config (constructor) | `params` keys (per-resolve) |
|---|---|---|---|
| `env` | built-in | none | `ref` (env var name) |
| `1password` | shells out to `op` CLI | `account`, `vault` (optional, defers to `op` config) | `ref` (op:// URI) |
| `keychain` | `keyring` crate | `service` (optional) | `ref` (item name) |
| `age` | `age` crate | `identity` (path to private key file) | `ref` (path to .age file) |
| `aws` | `aws-sdk-secretsmanager`, `aws-sdk-ssm` | `profile`, `region`, `role_arn` (all optional, defers to SDK credential chain) | `ref` (`sm://name` or `ssm:///path`), `field` (optional, for JSON secrets) |
| `hashicorp` | `vaultrs` | `address`, `token`, `namespace`, `mount` (defers to VAULT_ADDR/VAULT_TOKEN) | `ref` (secret path), `field` |
| `gcp` | `google-cloud-secretmanager-v1` | `project` (defers to Application Default Credentials) | `ref` (full resource name or short name) |
| `keyzero` | `reqwest` (HTTP client) | `endpoint`, `token` | `ref` (resource ID) |

### Feature Flags

Each provider is behind a cargo feature flag. Consumers compile only what they need:

```toml
[features]
default = ["env"]
onepassword = []
keychain = ["dep:keyring"]
age = ["dep:age"]
aws = ["dep:aws-sdk-secretsmanager", "dep:aws-sdk-ssm", "dep:aws-config"]
hashicorp = ["dep:vaultrs"]
gcp = ["dep:google-cloud-secretmanager-v1"]
keyzero = ["dep:reqwest"]
```

### keyzero Compatibility

The `secret-resolvers` crate is designed so keyzero can adopt it with minimal refactoring:

1. keyzero's 5 existing resolver implementations (env, vault, aws_sm, aws_sts, onepassword) move into `secret-resolvers`
2. They are refactored to take `ResolveRequest { params }` instead of keyzero's `ResolverConfig`
3. keyzero adds a thin mapping function in `pipeline.rs`:
   ```rust
   fn to_resolve_request(config: &ResolverConfig) -> ResolveRequest {
       let mut params = HashMap::new();
       if let Some(path) = &config.path { params.insert("path".into(), path.into()); }
       if let Some(field) = &config.field { params.insert("field".into(), field.into()); }
       // ...
       ResolveRequest { params }
   }
   ```
4. keyzero's orchestration fields (`name`, `mode`, `backend`, `credential_location`) stay in keyzero — they are pipeline concerns, not resolver concerns
5. keyzero's bundle YAML format does not change

**Risk mitigations for the keyzero migration:**

| Concern | Mitigation |
|---|---|
| `params` too simple for keyzero | `HashMap<String, toml::Value>` handles nested structures, arrays, typed values |
| Constructor differences | Constructors are per-provider, not trait-constrained — each takes what it needs |
| Async runtime | Both use tokio |
| Error types | Shared crate defines its own error type; both projects map to/from it |
| TTL | In the struct from day one; dotvault ignores it, keyzero uses it |

## Shell Hook

A small shell function that fires on directory change. If `.dotvault.toml` or `.dotvault.local.toml` exists in the current directory, it evals `dotvault export`.

Example (zsh):

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

Equivalent hooks for bash (PROMPT_COMMAND) and fish (variable event).

`dotvault hook --shell zsh|bash|fish` prints the snippet for the user to paste into their shell profile.

**v1 limitation:** env vars from a previous project linger when you `cd` out. Unset tracking can be added later.

## Secret Resolution Flow

1. Determine config file: if `.dotvault.local.toml` exists, use it. Otherwise use `.dotvault.toml`.
2. Parse the TOML config.
3. Instantiate providers: for each unique provider name in `[secrets]`, create an instance with config from `[providers.<name>]` (project-level, falling back to `~/.config/dotvault/config.toml`).
4. Resolve all secrets concurrently: for each entry in `[secrets]`, call the provider's `resolve()` with the entry's fields mapped into `ResolveRequest { params }`. Use `futures::join_all` to resolve in parallel — important for providers that make HTTP calls or shell out to CLIs.
5. If any secret fails to resolve, print a clear error per secret and exit non-zero. No partial injection.
6. Deliver secrets: either inject into subprocess env (`run`) or print `export` lines (`export`).

## Error Handling

- All secrets must resolve or dotvault exits non-zero
- Per-secret error messages: `Error resolving OPENAI_API_KEY (provider: 1password): op CLI not found`
- Provider not found: `Unknown provider 'foo' for secret BAR`
- Config parse errors: clear message with file path and line number
- Missing config file: `No .dotvault.toml or .dotvault.local.toml found in current directory`

## Distribution

Following the tooly project pattern (~/projects/tooly):

### Three installation channels

1. `cargo install dotvault`
2. `npm i -g @jondot/dotvault` — platform-specific binaries via optionalDependencies
3. Direct binary download from GitHub Releases

### npm package structure

- `@jondot/dotvault` — umbrella package with Node.js wrapper (platform detection + binary exec)
- `@jondot/dotvault-cli-darwin-arm64`
- `@jondot/dotvault-cli-linux-x64`
- `@jondot/dotvault-cli-linux-arm64`

Each platform package declares `os` and `cpu` constraints. npm installs only the matching binary.

### GitHub Actions release workflow

Triggered by `v*` tag push:

1. **Build phase** (matrix): macOS ARM64, Linux x64, Linux ARM64
   - Cross-compilation for Linux ARM64 via `gcc-aarch64-linux-gnu`
2. **Publish phase**: publishes all npm packages + creates GitHub Release with tarballs

### Version management

`scripts/bump-version.sh` updates Cargo.toml + all package.json files atomically.

## Testing Strategy

Weight is on integration tests with real (or locally emulated) backends.

### Integration tests (primary)

| Provider | CI strategy |
|---|---|
| `env` | Always available — set env vars in test |
| `keychain` | `keyring` crate's built-in mock credential store |
| `age` | Real — generate keypair in-memory, encrypt, resolve |
| `1password` | 1Password Connect server in Docker, or `op` service accounts |
| `aws` | LocalStack in Docker (Secrets Manager + SSM Parameter Store) |
| `hashicorp` | `vault server -dev` in Docker (in-memory, zero setup) |
| `gcp` | Fake HTTP server via `wiremock` crate (narrow API surface: create + access secret version) |
| `keyzero` | Fake HTTP server via `wiremock` (single `/v1/resolve` endpoint) |

Docker Compose for CI:

```yaml
services:
  localstack:
    image: localstack/localstack
  vault:
    image: hashicorp/vault
    command: server -dev
```

Each integration test:
- Seeds the test secret into the backend during setup
- Resolves via the real provider code path
- Is idempotent

### E2E tests (CLI harness)

Uses `assert_cmd` + `predicates` crates. Tests against the `env` provider (always available):

- Config parsing (`.dotvault.toml` and `.dotvault.local.toml` replacement semantics)
- Secret resolution through the full pipeline
- `dotvault run` — env var injection into subprocess
- `dotvault export` — output format
- Error cases (missing provider, bad reference, missing config file)

### Unit tests (thin)

Config parsing, error formatting, provider registry wiring. Not the providers themselves.

## keyzero Funnel Strategy

dotvault is designed as a standalone open-source tool that creates a natural upgrade path to keyzero.

### Shared crate (`secret-resolvers`)

- Published to crates.io independently
- Both dotvault and keyzero depend on it
- Contributors who add providers to dotvault automatically benefit keyzero
- Providers behind feature flags — each consumer compiles only what it needs

### The `keyzero` provider

dotvault includes a `keyzero` provider that talks to a keyzero server's `/v1/resolve` endpoint:

```toml
[providers.keyzero]
endpoint = "https://keyzero.mycompany.com"

[secrets]
API_KEY = { provider = "keyzero", ref = "db/password" }
```

Same dev experience, but the organization gets policy, audit, and secretless mode under the hood. The developer doesn't change their workflow.

### Growth path

1. dotvault catches fire as the simple, open-source, no-vendor-lock secret tool
2. Community contributes providers (shared crate)
3. Teams hit scale/policy/agent pain
4. `keyzero` provider = zero-friction upgrade
5. keyzero benefits from dotvault's provider ecosystem

### Community positioning

- dotvault is fully standalone — no keyzero dependency required
- README mentions keyzero as "for teams that need policy and agent access" — natural "what's next", not a sales pitch
