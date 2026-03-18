# dotvault

Secrets for developers. No setup, no vendor lock-in, no secrets on disk.

```bash
# Store a secret in your OS keychain
dv put --provider keychain --ref openai-key --value "sk-proj-abc123"

# Map it to an env var
dv add --name OPENAI_API_KEY --provider keychain --ref openai-key

# Run your app — secret is injected automatically
dv run -- npm start
```

## Zero-Friction Start

Your OS already has a secure secret store. Use it:

```bash
# Install
cargo install dotvault   # installs as 'dv'

# Store your API key in the OS keychain (macOS Keychain, Linux Secret Service)
dv put --provider keychain --ref my-openai-key --value "sk-proj-..."

# Init your project
cd my-project
dv init
dv add --name OPENAI_API_KEY --provider keychain --ref my-openai-key

# Done. Run your app.
dv run -- npm start
```

No Docker, no external services, no accounts to create. Works on macOS and Linux out of the box.

## Auto-Loading (Like direnv)

Add the hook to your shell and secrets appear when you `cd` into the project:

```bash
# Add to ~/.zshrc (or ~/.bashrc, ~/.config/fish/config.fish)
eval "$(dv hook --shell zsh)"
```

Then just `cd my-project` and your secrets are there.

## Team Workflow

VP R&D creates an OpenAI token and needs the team to use it:

```bash
# VP stores the token in 1Password (or Vault, or AWS, or the OS keychain)
dv put --provider keychain --ref team-openai-key --value "sk-proj-team-token"

# VP creates the project config
dv init
dv add --name OPENAI_API_KEY --provider keychain --ref team-openai-key
dv add --name DATABASE_URL --provider env --ref DATABASE_URL
git add .dotvault.toml && git commit -m "add secret mappings"
```

Developers clone and run:

```bash
dv run -- npm start
```

When the VP rotates the key:

```bash
dv put --provider keychain --ref team-openai-key --value "sk-proj-NEW-rotated-key"
```

Developers get the new value automatically. No Slack messages, no manual updates.

A developer who wants to use their own key:

```bash
dv add --local --name OPENAI_API_KEY --provider env --ref MY_PERSONAL_KEY
export MY_PERSONAL_KEY="sk-proj-my-own-key"
```

## Install

```bash
# cargo (installs as 'dv')
cargo install dotvault

# npm
npm i -g @jondot/dotvault

# or download from GitHub Releases
```

## Providers

Ordered by setup effort:

| Provider | Backend | Setup | Config |
|---|---|---|---|
| `keychain` | OS keychain (macOS Keychain, Linux Secret Service) | **None** | `service` (optional) |
| `env` | Environment variables | **None** | — |
| `1password` | 1Password CLI (`op`) | Install `op` CLI | `account`, `vault` |
| `age` | age-encrypted files in repo | Install `age` CLI | `identity` (key file path) |
| `hashicorp` | HashiCorp Vault | Running Vault server | `address`, `token`, `namespace` |
| `aws` | AWS Secrets Manager + SSM Parameter Store | AWS credentials | `region`, `profile`, `endpoint_url` |
| `gcp` | Google Cloud Secret Manager | GCP credentials | `project`, `endpoint_url` |
| `keyzero` | [keyzero](https://github.com/jondot/keyzero) server | Running keyzero | `endpoint`, `token` |

All provider config is optional — providers defer to their native CLI/SDK configuration by default.

**Writable providers** (`dv put`): `keychain`, `1password`, `aws`, `hashicorp`.

## Commands

```bash
dv run -- <cmd>           # Run a command with secrets injected
dv export                 # Print export KEY='VALUE' lines
dv add                    # Add a secret mapping (interactive)
dv put                    # Store a secret value in a vault (interactive)
dv init                   # Create starter .dotvault.toml
dv hook --shell zsh       # Print shell hook snippet
```

All commands support `--help` for full options. `add` and `put` support non-interactive mode via flags:

```bash
dv add --name API_KEY --provider keychain --ref my-key
dv add --local --name API_KEY --provider env --ref MY_ENV_VAR
dv put --provider keychain --ref my-key --value "secret-value"
```

## Config

### `.dotvault.toml` (checked into repo)

```toml
[secrets]
OPENAI_API_KEY = { provider = "keychain", ref = "team-openai-key" }
DATABASE_URL = { provider = "env", ref = "DATABASE_URL" }
SIGNING_KEY = { provider = "age", ref = "secrets/signing.key.age" }
```

With provider-specific config:

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[secrets]
DB_PASS = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_password" }
```

### `.dotvault.local.toml` (gitignored)

Personal overrides. **Completely replaces** `.dotvault.toml` when present — no merge:

```toml
[secrets]
OPENAI_API_KEY = { provider = "env", ref = "MY_PERSONAL_KEY" }
DATABASE_URL = { provider = "env", ref = "DATABASE_URL" }
```

### `~/.config/dotvault/config.toml` (global)

Provider config that applies across all projects. Project-level config takes precedence.

```toml
[providers.1password]
account = "my-team.1password.com"
```

## Reference Formats

| Provider | `ref` format | `field` |
|---|---|---|
| `keychain` | item name: `my-secret` | — |
| `env` | env var name: `MY_KEY` | — |
| `1password` | `op://Vault/Item/field` | — |
| `age` | path to `.age` file | — |
| `aws` | `sm://secret-name` or `ssm:///param/path` | optional, for JSON secrets |
| `hashicorp` | Vault path: `secret/data/myapp` | required |
| `gcp` | short name or `projects/X/secrets/Y/versions/Z` | — |
| `keyzero` | resource ID: `db/password` | — |

## Architecture

dotvault is a Cargo workspace with two crates:

- **`secret-resolvers`** — shared provider library, published to crates.io independently. Defines the `SecretResolver` trait and all provider implementations behind feature flags.
- **`dotvault`** — the CLI binary (installed as `dv`).

The `secret-resolvers` crate is designed to be shared with [keyzero](https://github.com/jondot/keyzero), a secretless platform for AI agents. If your team outgrows dotvault and needs policy-based access control, audit logging, or secretless agent access, the `keyzero` provider lets you upgrade without changing your workflow.

## License

MIT
