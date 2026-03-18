# dotvault

Resolve secrets from pluggable backends into your dev environment. No vendor lock-in, no secrets on disk.

```
.dotvault.toml → dotvault → [providers] → env vars → your app
```

## The Problem

Developers need secrets (API keys, tokens, connection strings) in local development. Every approach has drawbacks:

- **In code** — checked into git
- **In `.env` files** — clear text on disk
- **In shell profile** — not portable, visible
- **Manual export** — tedious, breaks on rotation

When a token gets rotated, someone has to redistribute it via Slack. Solutions like 1Password CLI work but create vendor lock-in.

## How It Works

Define what secrets your project needs and where they live:

```toml
# .dotvault.toml (checked into repo)
[providers.1password]
account = "my-team.1password.com"

[secrets]
OPENAI_API_KEY = { provider = "1password", ref = "op://Engineering/OpenAI/api-key" }
DATABASE_URL = { provider = "aws", ref = "sm://prod/database-url" }
```

Then either run your app through dotvault:

```bash
dotvault run -- npm start
```

Or add the shell hook and secrets appear automatically when you `cd` into the project:

```bash
# Add to ~/.zshrc
eval "$(dotvault hook --shell zsh)"
```

## Install

```bash
# npm
npm i -g @jondot/dotvault

# cargo
cargo install dotvault

# or download from GitHub Releases
```

## Providers

| Provider | Backend | Config |
|---|---|---|
| `env` | Environment variables | — |
| `1password` | 1Password CLI (`op`) | `account`, `vault` |
| `keychain` | OS keychain (macOS/Linux/Windows) | `service` |
| `age` | age-encrypted files | `identity` (key file path) |
| `aws` | AWS Secrets Manager + SSM Parameter Store | `region`, `profile`, `endpoint_url` |
| `hashicorp` | HashiCorp Vault | `address`, `token`, `namespace` |
| `gcp` | Google Cloud Secret Manager | `project`, `endpoint_url` |
| `keyzero` | [keyzero](https://github.com/jondot/keyzero) server | `endpoint`, `token` |

All provider config is optional — providers defer to their native CLI/SDK configuration by default.

## Config

### `.dotvault.toml` (checked into repo)

Defines secrets the project needs and where they come from:

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[secrets]
API_KEY = { provider = "1password", ref = "op://Engineering/OpenAI/api-key" }
DB_URL = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_url" }
DB_PASS = { provider = "aws", ref = "sm://prod/db-password" }
SIGNING_KEY = { provider = "age", ref = "secrets/signing.key.age" }
```

### `.dotvault.local.toml` (gitignored)

Personal overrides. **Completely replaces** `.dotvault.toml` when present — no merge:

```toml
[secrets]
API_KEY = { provider = "env", ref = "MY_PERSONAL_KEY" }
DB_URL = { provider = "env", ref = "LOCAL_DB_URL" }
```

### `~/.config/dotvault/config.toml` (global)

Provider config that applies across all projects:

```toml
[providers.1password]
account = "my-team.1password.com"

[providers.hashicorp]
address = "https://vault.mycompany.com"
```

Project-level provider config takes precedence over global.

## Commands

```bash
# Resolve secrets and run a command
dotvault run -- npm start

# Print export statements (for shell eval)
dotvault export

# Add a secret mapping (interactive)
dotvault add

# Add a secret mapping (non-interactive)
dotvault add --name OPENAI_API_KEY --provider 1password --ref "op://Engineering/OpenAI/api-key"

# Write a secret value into a vault (interactive)
dotvault put

# Write a secret value into a vault (non-interactive)
dotvault put --provider 1password --ref "op://Engineering/OpenAI/api-key" --value "sk-proj-abc123"

# Create a starter config
dotvault init

# Print shell hook for auto-loading
dotvault hook --shell zsh    # or bash, fish
```

### `dotvault add`

VP says "I put the OpenAI key under `op://Engineering/OpenAI/api-key` in 1Password." Developer runs:

```
$ dotvault add
Env var name: OPENAI_API_KEY
Provider: 1password
Reference: op://Engineering/OpenAI/api-key
✓ Added OPENAI_API_KEY to .dotvault.toml
```

Use `--local` to write to `.dotvault.local.toml` instead.

### `dotvault put`

VP creates a new OpenAI token and wants to store it in the team vault:

```
$ dotvault put
Provider: 1password
Reference: op://Engineering/OpenAI/api-key
Value (hidden): ****
✓ Stored secret at op://Engineering/OpenAI/api-key via 1password
```

Writable providers: `1password`, `keychain`, `aws`, `hashicorp`.

## Reference Formats

| Provider | `ref` format | `field` |
|---|---|---|
| `env` | env var name: `MY_KEY` | — |
| `1password` | `op://Vault/Item/field` | — |
| `keychain` | item name: `my-secret` | — |
| `age` | path to `.age` file | — |
| `aws` | `sm://secret-name` or `ssm:///param/path` | optional, for JSON secrets |
| `hashicorp` | Vault path: `secret/data/myapp` | required |
| `gcp` | short name or `projects/X/secrets/Y/versions/Z` | — |
| `keyzero` | resource ID: `db/password` | — |

## For Teams

The typical workflow:

1. Team lead sets up `.dotvault.toml` with shared secrets (1Password, Vault, AWS, etc.)
2. Developers clone the repo and run `dotvault run -- <their command>`
3. When secrets rotate, the source of truth updates — developers get new values automatically
4. Individual devs can use `.dotvault.local.toml` for personal overrides

## Architecture

dotvault is a Cargo workspace with two crates:

- **`secret-resolvers`** — shared provider library, published to crates.io independently. Defines the `SecretResolver` trait and all provider implementations behind feature flags.
- **`dotvault`** — the CLI binary.

The `secret-resolvers` crate is designed to be shared with [keyzero](https://github.com/jondot/keyzero), a secretless platform for AI agents. If your team outgrows dotvault and needs policy-based access control, audit logging, or secretless agent access, the `keyzero` provider lets you upgrade without changing your workflow.

## License

MIT
