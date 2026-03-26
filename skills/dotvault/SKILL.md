---
name: dotvault
description: Set up dotvault secrets management for any project — install the CLI, configure providers, map secrets, add shell hooks, and establish team workflows.
---

## Instructions

You are setting up dotvault for a project. dotvault resolves secrets from pluggable backends (OS keychain, 1Password, AWS, HashiCorp Vault, GCP, age-encrypted files, env vars) and injects them as environment variables — without ever storing secrets on disk.

Follow these steps, adapting to the project's needs.

### Step 1: Check if dotvault is installed

Run:
```
dv --version
```

If not installed, ask the user which method they prefer:

- **Cargo:** `cargo install dotvault` (installs as `dv`)
- **npm:** `npm i -g @jondot/dotvault`
- **GitHub Releases:** download from https://github.com/jondot/dotvault/releases

### Step 2: Understand the project

Before configuring, understand what secrets the project needs. Look for:

1. **Existing `.env` or `.env.example` files** — these list the env vars the project expects
2. **Docker compose files** — environment sections reveal required secrets
3. **Application config** — framework-specific config files (e.g., `database.yml`, `next.config.js`, `settings.py`)
4. **CI/CD config** — `.github/workflows/*.yml`, `Jenkinsfile`, etc. for secrets used in CI

Compile a list of secrets the project needs (e.g., `DATABASE_URL`, `API_KEY`, `AWS_SECRET_ACCESS_KEY`).

### Step 3: Choose providers

For each secret, recommend the best provider based on context:

| Use case | Provider | Why |
|----------|----------|-----|
| Personal dev keys | `keychain` | Zero setup, OS-native, supports biometric |
| Team-shared keys | `keychain` or `1password` | Keychain for small teams, 1Password for orgs |
| Cloud infrastructure | `aws` or `gcp` | Use what's already deployed |
| Self-hosted vault | `hashicorp` | For companies running Vault |
| Secrets checked into repo (encrypted) | `age` | Git-friendly, offline-capable |
| Existing env vars / CI | `env` | Pass-through, no migration needed |

**Default recommendation:** Start with `keychain` for most secrets. It requires zero setup and works on macOS and Linux.

### Step 4: Initialize the project

Run:
```
dv init
```

This creates a starter `.dotvault.toml`. Then edit it based on the secrets identified in Step 2.

### Step 5: Configure `.dotvault.toml`

Build the config file. Here are the patterns:

**Basic (single provider per type):**
```toml
[secrets]
DATABASE_URL = { provider = "keychain", ref = "myapp-db-url" }
API_KEY = { provider = "env", ref = "API_KEY" }
SIGNING_KEY = { provider = "age", ref = "secrets/signing.key.age" }
```

**With provider config:**
```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[providers.aws]
region = "us-east-1"
profile = "dev"

[secrets]
DB_PASS = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_password" }
S3_KEY = { provider = "aws", ref = "sm://prod/s3-credentials", field = "access_key" }
```

**Named providers (multiple instances of same type):**
```toml
[providers.secure]
type = "keychain"
biometric = true

[providers.prod-vault]
type = "hashicorp"
address = "https://vault.prod.mycompany.com"

[providers.staging-vault]
type = "hashicorp"
address = "https://vault.staging.mycompany.com"

[secrets]
SENSITIVE_TOKEN = { provider = "secure", ref = "my-token" }
PROD_DB = { provider = "prod-vault", ref = "secret/data/prod", field = "db_url" }
STAGING_DB = { provider = "staging-vault", ref = "secret/data/staging", field = "db_url" }
```

**Key rules:**
- `ref` is the provider-specific reference (item name for keychain, `op://...` for 1Password, `sm://...` or `ssm:///...` for AWS, path for age/hashicorp)
- `field` is required for `hashicorp`, optional for `aws` (JSON key extraction), unused for others
- Named providers use a `type` field to specify the actual provider type

### Step 6: Store initial secrets

For keychain-based secrets, store them:
```
dv put --provider keychain --ref <ref-name> --value "<secret-value>"
```

For biometric keychain:
```
dv put --provider secure --ref <ref-name> --value "<secret-value>"
```

Writing is supported for: `keychain`, `1password`, `aws`, `hashicorp`.

For other providers (age, env, gcp, keyzero), secrets are managed externally.

### Step 7: Test the setup

Verify secrets resolve:
```
dv export
```

This prints `export KEY='VALUE'` lines. Confirm all secrets appear. Then test with the project's actual run command:
```
dv run -- <the project's start command>
```

For example:
- `dv run -- npm start`
- `dv run -- python manage.py runserver`
- `dv run -- cargo run`

**Important:** If any secret fails to resolve, the entire command fails. Fix all secrets before proceeding.

### Step 8: Set up `.dotvault.local.toml` pattern

Add to `.gitignore`:
```
.dotvault.local.toml
```

Create an example file for developers:
```
# .dotvault.local.example.toml
# Copy to .dotvault.local.toml for personal overrides
# NOTE: This file COMPLETELY REPLACES .dotvault.toml (no merge)

[secrets]
# Example: use personal env var instead of team keychain
# DATABASE_URL = { provider = "env", ref = "MY_LOCAL_DB" }
```

### Step 9: Add shell hook (optional)

For automatic secret loading when entering the project directory (like direnv), suggest the shell hook:

**zsh (~/.zshrc):**
```
eval "$(dv hook --shell zsh)"
```

**bash (~/.bashrc):**
```
eval "$(dv hook --shell bash)"
```

**fish (~/.config/fish/config.fish):**
```
eval (dv hook --shell fish)
```

### Step 10: Update project documentation

Add a section to the project's README explaining:
1. Install dotvault (`cargo install dotvault` or `npm i -g @jondot/dotvault`)
2. Store required secrets (list the `dv put` commands or explain which external provider to configure)
3. Run the project with `dv run -- <command>`
4. Optional: set up shell hook for automatic loading
5. Optional: create `.dotvault.local.toml` for personal overrides

## Provider Reference

When the user asks about a specific provider, use this reference:

### keychain
- **Setup:** None (uses OS Keychain on macOS, Secret Service on Linux)
- **Config:** `service` (optional, defaults to "dotvault"), `biometric` (macOS Touch ID)
- **Ref format:** item name, e.g. `my-api-key`
- **Supports write:** Yes

### env
- **Setup:** None
- **Config:** None
- **Ref format:** environment variable name, e.g. `MY_API_KEY`
- **Supports write:** No

### 1password
- **Setup:** Install `op` CLI (`brew install 1password-cli`)
- **Config:** `account`, `vault`, `op_path`
- **Ref format:** `op://VaultName/ItemName/field`
- **Supports write:** Yes

### age
- **Setup:** Install `age` (`brew install age` or `cargo install age`)
- **Config:** `identity` (path to private key file)
- **Ref format:** path to `.age` file, e.g. `secrets/api.key.age`
- **Supports write:** No

### hashicorp
- **Setup:** Running Vault server
- **Config:** `address`, `token`, `namespace` (defers to `VAULT_ADDR` / `VAULT_TOKEN` env vars)
- **Ref format:** vault path, e.g. `secret/data/myapp`
- **Field:** Required
- **Supports write:** Yes

### aws
- **Setup:** AWS credentials configured (CLI, IAM role, etc.)
- **Config:** `region`, `profile`, `endpoint_url`
- **Ref format:** `sm://secret-name` (Secrets Manager) or `ssm:///param/path` (Parameter Store)
- **Field:** Optional (extracts key from JSON)
- **Supports write:** Yes

### gcp
- **Setup:** GCP credentials (`gcloud auth` or `GOOGLE_APPLICATION_CREDENTIALS`)
- **Config:** `project`, `endpoint_url`
- **Ref format:** `my-secret` (short, requires project) or `projects/X/secrets/Y/versions/Z` (full)
- **Supports write:** No

### keyzero
- **Setup:** Running keyzero server
- **Config:** `endpoint` (required), `token` (optional)
- **Ref format:** resource ID, e.g. `db/password`
- **Supports write:** No

## Key Concepts

- **No secrets on disk:** dotvault never writes secrets to files. They're resolved at runtime from backends.
- **`.dotvault.local.toml` completely replaces `.dotvault.toml`** — it does NOT merge. Use it for full personal overrides.
- **All-or-nothing resolution:** if any secret fails, the entire command fails. No partial injection.
- **Concurrent resolution:** all secrets resolve in parallel for speed.
- **Provider config is optional:** providers defer to their native configuration (env vars, CLI config) by default.
- **Global config** at `~/.config/dotvault/config.toml` applies to all projects. Project config takes precedence.
