---
name: dotvault-run
description: Use dotvault to run processes with secrets — discover available secrets, run commands with least-privilege using --only, export specific values, and handle resolution failures. Use when a project has .dotvault.toml and you need to run something that requires secrets.
---

## Instructions

You are an agent working in a project that uses dotvault for secrets management. A `.dotvault.toml` (or `.dotvault.local.toml`) file exists in the project. Your job is to run processes with the minimum secrets they need.

**Principle: least privilege.** Always use `--only` to limit which secrets a process can see. Never inject all secrets when a process only needs one.

### Step 1: Discover available secrets

Read `.dotvault.toml` (or `.dotvault.local.toml` if it exists — it takes priority). Look at the `[secrets]` section to see what's available:

```toml
[secrets]
DATABASE_URL = { provider = "keychain", ref = "myapp-db-url" }
API_KEY = { provider = "keychain", ref = "myapp-api-key" }
REDIS_URL = { provider = "env", ref = "REDIS_URL" }
```

Each key (e.g., `DATABASE_URL`) is the env var name that will be injected. You don't need to know the provider details — just the key names.

If the project uses per-environment sections (`[production.secrets]`, `[staging.secrets]`), the bare `[secrets]` section is the development environment. dotvault auto-selects based on `DOTVAULT_ENV`, `NODE_ENV`, etc.

### Step 2: Run a process with specific secrets

Use `--only` to inject only the secrets the process needs:

```bash
# Single secret
dv run --only DATABASE_URL -- psql

# Multiple secrets (comma-separated)
dv run --only DATABASE_URL,REDIS_URL -- node server.js

# All secrets (only when the process genuinely needs all of them)
dv run -- npm start
```

**Always prefer `--only`** unless the process is the main application entry point that legitimately needs every secret.

### Step 3: Get a secret value

If you need a secret value (e.g., to pass as a CLI argument rather than an env var):

```bash
# Export a specific secret
dv export --only API_KEY
# Output: export API_KEY='the-value'
```

**Do not** use `dv export` without `--only` when you only need one value.

### When resolution fails

If `dv run` or `dv export` fails with a resolution error:

- **"secret 'X': resolution failed"** — The secret can't be fetched from its provider. The user needs to configure the provider or run `dv put --missing` to fill it in. Tell the user.
- **"secret 'X': resolved to empty value"** — The provider returned an empty string. Either the secret hasn't been set, or it needs `allow_empty = true` in the config. Tell the user.
- **"secret(s) not found in config: X"** — The key you passed to `--only` doesn't exist in `.dotvault.toml`. Check the config for the correct key name.

**Do not** attempt to fix secrets yourself. Tell the user what failed and suggest `dv put --missing`.

### Quick reference

| Task | Command |
|------|---------|
| Run with specific secrets | `dv run --only KEY1,KEY2 -- cmd` |
| Run with all secrets | `dv run -- cmd` |
| Export specific secret | `dv export --only KEY` |
| Export all secrets | `dv export` |
| Fill missing secrets | `dv put --missing` |
| Check what's available | Read `.dotvault.toml` `[secrets]` section |
