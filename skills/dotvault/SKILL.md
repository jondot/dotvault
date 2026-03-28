---
name: dotvault
description: Set up and maintain dotvault secrets management for any project — install the CLI, configure providers, map secrets per environment, add shell hooks, integrate with package managers, and keep configuration in sync as the project evolves. Run this skill any time to detect drift.
---

## Instructions

You are setting up or maintaining dotvault for a project. dotvault resolves secrets from pluggable backends (OS keychain, 1Password, AWS, HashiCorp Vault, GCP, age-encrypted files, env vars) and injects them as environment variables — without ever storing secrets on disk.

**This skill is designed to be run repeatedly.** It works for initial setup and for ongoing maintenance as the project evolves. Each step detects current state and only acts on what's missing or out of date.

### Step 1: Assess current state

First, determine what's already in place:

1. **Check if dotvault is installed:** Run `dv --version`
2. **Check for existing config:** Look for `.dotvault.toml` and `.dotvault.local.toml`
3. **Check for `.gitignore` entry:** Is `.dotvault.local.toml` already ignored?
4. **Check for project integration:** If Node.js, is `@jondot/dotvault` in devDependencies? Are scripts prefixed?

If dotvault is not installed, ask the user which method they prefer:
- **Cargo:** `cargo install dotvault` (installs as `dv`)
- **npm:** `npm i -g @jondot/dotvault`
- **GitHub Releases:** download from https://github.com/jondot/dotvault/releases

If `.dotvault.toml` already exists, read it — you'll compare its secrets against what the project actually needs in the next step.

### Step 2: Discover secrets and plan mappings interactively

Scan the project for all secrets it expects. Look for:

1. **Existing `.env` or `.env.example` files** — these list the env vars the project expects
2. **Docker compose files** — environment sections reveal required secrets
3. **Application config** — framework-specific config files (e.g., `database.yml`, `next.config.js`, `settings.py`)
4. **CI/CD config** — `.github/workflows/*.yml`, `Jenkinsfile`, etc. for secrets used in CI
5. **Source code** — `process.env.X`, `os.environ["X"]`, `env::var("X")` references

Compile a full list of secrets the project needs (e.g., `DATABASE_URL`, `API_KEY`, `AWS_SECRET_ACCESS_KEY`).

**If a `.dotvault.toml` already exists,** compare the discovered secrets against what's already mapped. Identify:
- **New secrets** — discovered in the project but not in `.dotvault.toml`
- **Stale secrets** — in `.dotvault.toml` but no longer referenced by the project
- **Missing environment coverage** — secrets mapped for development but not for production/staging, or vice versa

Report your findings to the user before making changes. If everything is in sync and no gaps are found, say so and stop — don't re-do steps that are already complete.

#### Determine environments

Ask the user which environments to configure:

> "Which environments should I configure? I'll set up **development** by default. Should I also add production, staging, or others?"

- Default: development only
- If CI/CD config exists (`.github/workflows/`, `Jenkinsfile`, etc.), suggest adding a `production` or `ci` environment as well
- Wait for the user's answer before proceeding

#### Interactive mapping — per environment

For each environment the user confirmed, present a numbered table of all discovered secrets with recommended defaults, then loop until the user is satisfied.

**Present the table:**

| # | Secret | Provider | Ref | Allow Empty |
|---|--------|----------|-----|-------------|
| 1 | DATABASE_URL | keychain | myapp-database-url | false |
| 2 | API_KEY | keychain | myapp-api-key | false |
| 3 | DEBUG_TOKEN | env | DEBUG_TOKEN | true |

**Recommendation defaults:**
- Use `keychain` for most secrets (zero setup, OS-native)
- Use `env` for pass-through/CI/debug values or secrets that are acceptable empty
- Set `allow_empty = true` for optional secrets (debug flags, feature toggles, non-critical tokens)
- If an existing `.dotvault.toml` already maps a secret, use those values as the starting defaults

**User interaction loop:**

After displaying the table, prompt:

> "Enter a number to change its provider, ref, or allow_empty — or type **done** to confirm."

- Accept input like `2 provider=env ref=MY_API_KEY` or `3 allow_empty=false`
- Re-display the updated table after each change
- Repeat until the user types `done`

**For subsequent environments** (e.g., production after development):

- **Discover existing env vars on the platform first.** Before presenting the mapping table, ask the user what platform the environment runs on and run the appropriate CLI command to list existing env var names:

  | Platform | Command |
  |----------|---------|
  | Heroku | `heroku config --shell --app <app> \| cut -d= -f1` |
  | Vercel | `vercel env ls <environment>` |
  | Fly.io | `fly secrets list --app <app>` |
  | Railway | `railway variables` |
  | Render | `render env list` |
  | AWS ECS | Check task definition environment variables |
  | Kubernetes | `kubectl get secret <name> -o jsonpath='{.data}' \| jq 'keys'` |
  | Docker Compose | Read `environment:` section in `docker-compose.yml` |
  | Generic / SSH | `ssh <host> env \| cut -d= -f1 \| sort` |

  Use the discovered env var names to pre-fill the production table — each one becomes `provider = "env"` with `ref` set to the var name (passthrough). Merge with any secrets from the development mapping that weren't found on the platform.

  If the user doesn't know or the platform isn't listed, fall back to copying from the previous environment's mappings.

- Adjust recommendations: flip `keychain` → `env` for production (CI/CD environments typically inject secrets via env vars)
- Let the user adjust again before confirming

**After all environments are confirmed**, proceed to write the config in Step 5. Do not alter provider, ref, or allow_empty values — use exactly what the user confirmed.

### Step 3: Choose providers

Provider selection happens interactively in Step 2. This reference table is provided for context when answering user questions or when a secret's use case isn't obvious from its name:

| Use case | Provider | Why |
|----------|----------|-----|
| Personal dev keys | `keychain` | Zero setup, OS-native, supports biometric |
| Team-shared keys | `keychain` or `1password` | Keychain for small teams, 1Password for orgs |
| Cloud infrastructure | `aws` or `gcp` | Use what's already deployed |
| Self-hosted vault | `hashicorp` | For companies running Vault |
| Secrets checked into repo (encrypted) | `age` | Git-friendly, offline-capable |
| Existing env vars / CI | `env` | Pass-through, no migration needed |

**Default recommendation:** Start with `keychain` for most secrets. It requires zero setup and works on macOS and Linux. Use `env` for pass-through or CI secrets. The interactive table in Step 2 applies these defaults automatically — only consult this step if the user asks about a specific provider or use case.

### Step 4: Initialize or update the config

**If no `.dotvault.toml` exists,** run:
```
dv init
```

**If `.dotvault.toml` already exists,** skip init and proceed to update it with any new or changed secrets identified in Step 2.

### Step 5: Configure `.dotvault.toml`

Write the mappings confirmed by the user in Step 2. The provider, ref, and `allow_empty` values come directly from the interactive session — do not change them.

Add new secret mappings and remove stale ones. Here are the patterns:

**Basic (single provider per type):**
```toml
[secrets]
DATABASE_URL = { provider = "keychain", ref = "myapp-db-url" }
API_KEY = { provider = "env", ref = "API_KEY" }
SIGNING_KEY = { provider = "age", ref = "secrets/signing.key.age" }
DEBUG_TOKEN = { provider = "env", ref = "DEBUG_TOKEN", allow_empty = true }
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

**Per-environment secrets:**

Use environment-specific sections to define different secrets per environment. The bare `[secrets]` section is the `development` environment. Other environments use `[<env>.secrets]`:

```toml
[providers.hashicorp]
address = "https://vault.mycompany.com"

# Development (default) — bare [secrets] = development
[secrets]
API_KEY = { provider = "keychain", ref = "my-api-key" }
DB_URL = { provider = "env", ref = "DEV_DB_URL" }

# Production
[production.secrets]
API_KEY = { provider = "env", ref = "API_KEY" }
DB_URL = { provider = "env", ref = "DATABASE_URL" }

# Staging
[staging.secrets]
API_KEY = { provider = "hashicorp", ref = "secret/data/myapp", field = "api_key" }
DB_URL = { provider = "hashicorp", ref = "secret/data/myapp", field = "db_url" }
```

dotvault auto-detects the current environment from these env vars (in priority order):
1. `DOTVAULT_ENV` (explicit override, highest priority)
2. `NODE_ENV`
3. `RAILS_ENV`
4. `APP_ENV`
5. `RACK_ENV`
6. Default: `development`

To run with a specific environment:
```
DOTVAULT_ENV=staging dv run -- npm start
```

**Per-environment rules:**
- Provider config (`[providers.*]`) is shared across all environments — no duplication needed
- You cannot have both `[secrets]` and `[development.secrets]` — bare `[secrets]` already means development
- If environment sections exist but the detected environment is missing, dotvault errors (all-or-nothing)
- Use full environment names: `development`, `production`, `staging` (matches framework conventions)

**Key rules:**
- `ref` is the provider-specific reference (item name for keychain, `op://...` for 1Password, `sm://...` or `ssm:///...` for AWS, path for age/hashicorp)
- `field` is required for `hashicorp`, optional for `aws` (JSON key extraction), unused for others
- Named providers use a `type` field to specify the actual provider type

### Step 6: Fill missing secrets

After configuring the mappings, tell the user to fill in any secrets that can't yet be resolved:

```
dv put --missing
```

This scans the config, tries to resolve each secret, and interactively prompts the user to provide values for any that fail. It:
- Shows each missing secret with its provider, ref, and field
- Skips read-only providers (`env`, `age`, `gcp`, `keyzero`) with a note to configure them externally
- Accepts hidden input for each value (empty to skip)
- Writes directly to the correct provider

**Do NOT run `dv put` commands on behalf of the user.** Your job is to set up the complete `.dotvault.toml` mappings for all environments. The user then runs `dv put --missing` themselves to fill in the actual secret values.

For manual one-off writes, the user can also run:
```
dv put --provider keychain --ref <ref-name> --value "<secret-value>"
```

Writing is supported for: `keychain`, `1password`, `aws`, `hashicorp`.

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

**Important:** If any secret fails to resolve, the entire command fails. Run `dv put --missing` again to fill any remaining gaps. Note that secrets resolving to an **empty string** also count as failures by default — set `allow_empty = true` in the config for any secret that is permitted to be empty.

### Step 8: Set up `.dotvault.local.toml` pattern

**Skip if already set up** (`.dotvault.local.toml` is in `.gitignore` and `.dotvault.local.example.toml` exists). If the config changed, update `.dotvault.local.example.toml` to reflect new secrets.

Add to `.gitignore` (if not already there):
```
.dotvault.local.toml
```

Create or update `.dotvault.local.example.toml` for developers. If the project has secrets with hardcoded defaults in development (e.g., from `.env.example`, docker-compose defaults, or framework config), include those as **commented-out mappings** so developers can see what's available and uncomment to override:

```toml
# .dotvault.local.example.toml
# Copy to .dotvault.local.toml for personal overrides
# NOTE: This file COMPLETELY REPLACES .dotvault.toml (no merge)

[secrets]
# These have defaults in the development config — uncomment to override:
# DATABASE_URL = { provider = "env", ref = "MY_LOCAL_DB" }
# REDIS_URL = { provider = "env", ref = "MY_LOCAL_REDIS" }

# These have no defaults — you must configure them:
# API_KEY = { provider = "keychain", ref = "myapp-api-key" }
```

When generating this file, look at the development environment's `[secrets]` section and include commented versions of any secrets that already have working defaults (e.g., `env` refs pointing to vars with known default values, or secrets the project can run without). Separate them from secrets that have no defaults and must be provided.

### Step 9: Add shell hook (optional)

**Skip if already asked or already configured.** Only offer on first setup.

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

### Step 10: Node.js project integration (if applicable)

**If the project has a `package.json`:**

Check if dotvault is already integrated (look for `@jondot/dotvault` in devDependencies and `dv run` in scripts).

**If not yet integrated,** ask the user:

> "Would you like me to integrate dotvault into your package.json scripts, or will you run `dv run` manually when needed?"

If they want integration:

1. **Add dotvault as a dev dependency** (if not already present):
   ```
   npm i -D @jondot/dotvault
   ```

2. **Prefix scripts that need secrets with `dv run --`.**  Read `package.json` and identify scripts that would need environment secrets (e.g., `dev`, `start`, `build`, `test`, `migrate`, `seed` — anything that runs the application or talks to external services). Do NOT prefix scripts that are purely local tooling (e.g., `lint`, `format`, `typecheck`). Do NOT double-prefix scripts already using `dv run`.

   For example, if the current scripts are:
   ```json
   {
     "scripts": {
       "dev": "next dev",
       "build": "next build",
       "start": "next start",
       "lint": "eslint .",
       "migrate": "prisma migrate deploy"
     }
   }
   ```

   Update them to:
   ```json
   {
     "scripts": {
       "dev": "dv run -- next dev",
       "build": "dv run -- next build",
       "start": "dv run -- next start",
       "lint": "eslint .",
       "migrate": "dv run -- prisma migrate deploy"
     }
   }
   ```

   Use your judgement per script — if a script clearly doesn't need secrets, leave it alone.

**If already integrated,** check for new scripts added since last run that need secrets but aren't prefixed. Update those only.

### Step 11: Update project documentation

**Skip if the README already has a dotvault section** and it's still accurate. If the config changed (new secrets, new environments), update the docs section to reflect current state.

Add or update a section in the project's README explaining:
1. Install dotvault (`cargo install dotvault` or `npm i -g @jondot/dotvault`)
2. Run `dv put --missing` to fill in any secrets you don't have yet
3. Run the project with `dv run -- <command>`
4. Optional: set up shell hook for automatic loading
5. Optional: create `.dotvault.local.toml` for personal overrides

### Summary output

After completing all steps, print a brief summary of what was done and what was already up to date. For example:

```
dotvault status:
  ✓ CLI installed (v0.5.0)
  ✓ Config: 2 new secrets added, 1 stale removed
  ✓ Environments: development, production, staging
  ✓ Node.js: 1 new script prefixed (migrate)
  ✓ .gitignore: already configured
  ⚠ Run `dv put --missing` to fill 3 unresolved secrets
```

This helps the user understand what changed, especially on repeat runs.

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
- **Per-environment secrets:** Define different secrets per environment (`[secrets]` for development, `[production.secrets]`, `[staging.secrets]`, etc.). Environment is auto-detected from `DOTVAULT_ENV`, `NODE_ENV`, `RAILS_ENV`, `APP_ENV`, or `RACK_ENV` — defaulting to `development`. Providers are shared across all environments.
- **`.dotvault.local.toml` completely replaces `.dotvault.toml`** — it does NOT merge. Use it for full personal overrides.
- **All-or-nothing resolution:** if any secret fails, the entire command fails. No partial injection.
- **Concurrent resolution:** all secrets resolve in parallel for speed.
- **Provider config is optional:** providers defer to their native configuration (env vars, CLI config) by default.
- **Global config** at `~/.config/dotvault/config.toml` applies to all projects. Project config takes precedence.
- **Empty value rejection:** By default, secrets that resolve to an empty string are treated as errors. Set `allow_empty = true` on individual secrets to permit empty values.
