# dotvault

Secrets for developers. No setup, no vendor lock-in, no secrets on disk.

## Quick Start

### Install

```bash
cargo install dotvault   # installs as 'dv'

# or via npm
npm i -g @jondot/dotvault

# or download from GitHub Releases
```

### First project

```bash
cd my-project
dv init                                                          # creates .dotvault.toml
dv put --provider keychain --ref my-api-key --value "sk-..."     # store a secret
dv add --name API_KEY --provider keychain --ref my-api-key       # map it to an env var
dv run -- npm start                                              # run with secrets injected
```

* `put` writes a secret to your given provider
* `add` maps it within the current project
* `run` will fetch all the mappings and make them available for the process

### Auto-loading

Add the hook to your shell and secrets load when you `cd` into a project:

```bash
# zsh
eval "$(dv hook --shell zsh)"

# bash
eval "$(dv hook --shell bash)"

# fish
dv hook --shell fish | source
```

## Usage

### Commands

```bash
dv run -- <cmd>           # Run a command with secrets injected
dv export                 # Print export KEY='VALUE' lines
dv add                    # Add a secret mapping (interactive)
dv put                    # Store a secret value in a vault (interactive)
dv init                   # Create starter .dotvault.toml
dv hook --shell zsh       # Print shell hook snippet
```

`add` and `put` accept flags for non-interactive use:

```bash
dv add --name API_KEY --provider keychain --ref my-key
dv add --local --name API_KEY --provider env --ref MY_ENV_VAR
dv put --provider keychain --ref my-key --value "secret-value"
```

### Providers

| Provider | Backend | Setup |
|---|---|---|
| `keychain` | OS keychain (macOS Keychain, Linux Secret Service) | None |
| `env` | Environment variables | None |
| `1password` | 1Password CLI (`op`) | Install `op` CLI |
| `age` | age-encrypted files in repo | Install `age` CLI |
| `hashicorp` | HashiCorp Vault | Running Vault server |
| `aws` | AWS Secrets Manager + SSM Parameter Store | AWS credentials |
| `gcp` | Google Cloud Secret Manager | GCP credentials |
| `keyzero` | [keyzero](https://github.com/jondot/keyzero) server | Running keyzero |

Writable providers (`dv put`): `keychain`, `1password`, `aws`, `hashicorp`.

### Reference formats

| Provider | `ref` format | `field` |
|---|---|---|
| `keychain` | item name: `my-secret` | -- |
| `env` | env var name: `MY_KEY` | -- |
| `1password` | `op://Vault/Item/field` | -- |
| `age` | path to `.age` file | -- |
| `aws` | `sm://secret-name` or `ssm:///param/path` | optional (JSON secrets) |
| `hashicorp` | Vault path: `secret/data/myapp` | required |
| `gcp` | short name or `projects/X/secrets/Y/versions/Z` | -- |
| `keyzero` | resource ID: `db/password` | -- |

## Configuration

### `.dotvault.toml`

Checked into the repo. Maps environment variable names to provider references:

```toml
[secrets]
API_KEY = { provider = "keychain", ref = "team-api-key" }
DATABASE_URL = { provider = "env", ref = "DATABASE_URL" }
VAULT_SECRET = { provider = "hashicorp", ref = "secret/data/myapp", field = "token" }
```

### Provider configuration

Provider-specific settings go in a `[providers]` section:

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[providers.1password]
account = "my-team.1password.com"

[secrets]
DB_PASS = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_password" }
```

### Named providers

Multiple instances of the same provider type, using the `type` field:

```toml
[providers.secure]
type = "keychain"
biometric = true          # Touch ID on macOS

[providers.prod-vault]
type = "hashicorp"
address = "https://vault.prod.mycompany.com"

[secrets]
SENSITIVE_TOKEN = { provider = "secure", ref = "my-token" }
DB_PASS = { provider = "prod-vault", ref = "secret/data/app", field = "db_password" }
```

If no `type` field is set, the provider name is used as the type.

### `.dotvault.local.toml`

Personal overrides, gitignored. Completely replaces `.dotvault.toml` when present:

```toml
[secrets]
API_KEY = { provider = "env", ref = "MY_PERSONAL_KEY" }
DATABASE_URL = { provider = "env", ref = "DATABASE_URL" }
```

### `~/.config/dotvault/config.toml`

Global provider config that applies across all projects. Project-level config takes precedence.

## Secret Rotation

A typical rotation workflow using HashiCorp Vault.

**Manager** stores the initial secret:

```bash
dv put --provider hashicorp --ref secret/data/myapp --field api_key --value "key-v1"
```

The team shares a `.dotvault.toml`:

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[secrets]
API_KEY = { provider = "hashicorp", ref = "secret/data/myapp", field = "api_key" }
```

**Developers** clone the repo and run:

```bash
dv run -- npm start   # fetches api_key from Vault
```

When the manager rotates the key:

```bash
dv put --provider hashicorp --ref secret/data/myapp --field api_key --value "key-v2"
```

Every developer gets the new value on their next run. No Slack messages, no manual updates, no redeployment of config.

## Environments

A single `.dotvault.toml` can define different secret mappings per environment. Environment names use full words to match framework conventions: `development`, `production`, `staging`.

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

[secrets]
API_KEY = { provider = "keychain", ref = "my-api-key" }
DB_URL = { provider = "env", ref = "DEV_DB_URL" }

[production.secrets]
API_KEY = { provider = "env", ref = "API_KEY" }
DB_URL = { provider = "env", ref = "DATABASE_URL" }

[staging.secrets]
API_KEY = { provider = "hashicorp", ref = "secret/data/myapp", field = "api_key" }
DB_URL = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_url" }
```

The bare `[secrets]` section is the `development` environment.

### Environment detection

dotvault checks these environment variables in order and uses the first non-empty value:

1. `DOTVAULT_ENV`
2. `NODE_ENV`
3. `RAILS_ENV`
4. `APP_ENV`
5. `RACK_ENV`
6. Default: `development`

On a developer's machine with no env set, `[secrets]` is used. In production where `NODE_ENV=production`, `[production.secrets]` is selected automatically.

```bash
# Explicit override
DOTVAULT_ENV=staging dv run -- npm start
```

### Development vs. production

In development, secrets come from local tools -- keychain, 1Password, env vars. Developers mix and match.

In production, secrets typically come from the platform. The `env` provider passes through environment variables that are already injected by your CI/CD or orchestrator:

```toml
[production.secrets]
API_KEY = { provider = "env", ref = "API_KEY" }
DB_URL = { provider = "env", ref = "DATABASE_URL" }
SIGNING_KEY = { provider = "env", ref = "SIGNING_KEY" }
```

Same `dv run` command in `package.json`, different wiring per environment.

## Claude Code Skill

dotvault ships a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) skill that lets Claude set up secrets management for any project.

```bash
claude plugin add @jondot/dotvault
```

Then run `/dotvault` and Claude will walk through the full setup.

## License

MIT
