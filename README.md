<div align="center">

# dotvault

**Secrets for code agents and developers. No setup, no vendor lock-in, no secrets on disk.**

[![Crates.io](https://img.shields.io/crates/v/dotvault.svg)](https://crates.io/crates/dotvault)
[![CI](https://github.com/jondot/dotvault/actions/workflows/ci.yml/badge.svg)](https://github.com/jondot/dotvault/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Install](#install) | [With Claude Code](#with-claude-code) | [For Developers](#for-developers) | [Providers](#providers)

</div>

## Install

```bash
cargo install dotvault    # installs as 'dv'

# or
npm i -g @jondot/dotvault
```

## With Claude Code

Two commands to install the plugin, then type `/dotvault` in any project:

```
/plugin marketplace add jondot/dotvault
/plugin install dotvault@jondot-dotvault
```

Claude scans your project -- `.env` files, docker-compose, framework configs, source code -- finds every secret it needs, and asks you how to map each one. Pick a provider (Keychain, 1Password, AWS, whatever you use), and Claude wires up everything: config, `.gitignore`, package scripts.

From then on, when Claude runs your app it resolves only the secrets that command needs. No `.env` files on disk. No secrets leaking into the AI's context window. Run `/dotvault` again any time -- it detects drift and only fixes what changed.

## For Developers

**Replace `.env` files.** Same workflow, but secrets come from your vault instead of a plaintext file:

```bash
dv init                                                        # creates .dotvault.toml
dv put --provider keychain --ref my-api-key --value "sk-..."   # store in OS keychain
dv add --name API_KEY --provider keychain --ref my-api-key     # map it to an env var
dv run -- npm start                                            # run with secrets injected
```

**Replace `direnv`.** Auto-load secrets when you `cd` into a project:

```bash
eval "$(dv hook --shell zsh)"    # or bash, fish
```

**Replace sharing secrets over Slack.** Commit `.dotvault.toml` to the repo. One dev uses Keychain, another uses 1Password, production uses platform env vars -- same config, everyone resolves from their own vault:

```toml
[secrets]
API_KEY = { provider = "keychain", ref = "my-api-key" }
DB_URL  = { provider = "env", ref = "DEV_DB_URL" }

[production.secrets]
API_KEY = { provider = "env", ref = "API_KEY" }
DB_URL  = { provider = "env", ref = "DATABASE_URL" }
```

Environment auto-detected from `DOTVAULT_ENV`, `NODE_ENV`, `RAILS_ENV`, or `APP_ENV`.

## Providers

| Provider | What it wraps | Setup |
|---|---|---|
| `keychain` | macOS Keychain, Linux Secret Service | None |
| `env` | Environment variables | None |
| `1password` | 1Password CLI (`op`) | Install `op` |
| `age` | age-encrypted files in repo | Install `age` |
| `hashicorp` | HashiCorp Vault | Running server |
| `aws` | Secrets Manager + SSM Parameter Store | AWS credentials |
| `gcp` | Google Cloud Secret Manager | GCP credentials |
| `keyzero` | [keyzero](https://github.com/jondot/keyzero) | Running server |

## Commands

```
dv run [--only KEY,...] [--clean-env] -- <cmd>    Run with secrets injected
dv export [--format json]                         Print secrets as shell exports or JSON
dv status [--format json]                         Show which secrets resolve
dv validate [--format json]                       Check config without resolving
dv add                                            Add a secret mapping
dv put                                            Store a secret in a vault
dv init                                           Create .dotvault.toml
dv hook --shell <zsh|bash|fish>                   Print shell hook
```

## Contributing

Contributions welcome! Open an issue or submit a PR.

## License

MIT
