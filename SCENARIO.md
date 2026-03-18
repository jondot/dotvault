# dotvault Testing Scenarios

Manual testing scenarios for your local dev machine (macOS ARM64, Docker available, no `op` CLI).

All commands assume you're in a fresh temp directory. Set up once:

```bash
export DV=/Users/jondot/projects/dotvault/target/release/dotvault
mkdir -p /tmp/dotvault-test && cd /tmp/dotvault-test
```

Rebuild if needed: `cargo build --release -p dotvault` from the dotvault repo.

---

## Scenario 1: Basic env provider — the happy path

Tests the core flow: init, add a mapping, export, run.

```bash
cd /tmp/dotvault-test && mkdir s1 && cd s1

# 1. Init a new project
$DV init
# Expected: "created /tmp/dotvault-test/s1/.dotvault.toml"
cat .dotvault.toml
# Expected: starter template with comments

# 2. Add a secret mapping (non-interactive)
$DV add --name OPENAI_API_KEY --provider env --ref MY_OPENAI_KEY

# 3. Verify the config was updated
cat .dotvault.toml
# Expected: [secrets] section with OPENAI_API_KEY = { provider = "env", ref = "MY_OPENAI_KEY" }

# 4. Set the env var that the mapping points to
export MY_OPENAI_KEY="sk-test-12345"

# 5. Export and verify
$DV export
# Expected: export OPENAI_API_KEY='sk-test-12345'

# 6. Run a command with injected secrets
$DV run -- env | grep OPENAI_API_KEY
# Expected: OPENAI_API_KEY=sk-test-12345

# 7. Run a real command
$DV run -- sh -c 'echo "My key is: $OPENAI_API_KEY"'
# Expected: My key is: sk-test-12345
```

## Scenario 2: Multiple secrets from env provider

```bash
cd /tmp/dotvault-test && mkdir s2 && cd s2

# Write config directly
cat > .dotvault.toml << 'EOF'
[secrets]
DB_URL = { provider = "env", ref = "TEST_DB_URL" }
API_KEY = { provider = "env", ref = "TEST_API_KEY" }
REDIS_URL = { provider = "env", ref = "TEST_REDIS" }
EOF

export TEST_DB_URL="postgres://localhost:5432/mydb"
export TEST_API_KEY="key-abc-123"
export TEST_REDIS="redis://localhost:6379"

# All three should appear
$DV export
# Expected: three export lines

$DV run -- sh -c 'echo "$DB_URL | $API_KEY | $REDIS_URL"'
# Expected: postgres://localhost:5432/mydb | key-abc-123 | redis://localhost:6379
```

## Scenario 3: Local config replaces shared

Simulates a developer overriding the team config with personal settings.

```bash
cd /tmp/dotvault-test && mkdir s3 && cd s3

# Team config (shared, checked in)
cat > .dotvault.toml << 'EOF'
[secrets]
API_KEY = { provider = "env", ref = "TEAM_KEY" }
DB_URL = { provider = "env", ref = "TEAM_DB" }
EOF

# Developer's local override (gitignored)
cat > .dotvault.local.toml << 'EOF'
[secrets]
API_KEY = { provider = "env", ref = "MY_KEY" }
EOF

export TEAM_KEY="team-value"
export TEAM_DB="team-db-url"
export MY_KEY="my-personal-key"

# Should use local config ONLY (complete replacement, not merge)
$DV export
# Expected: ONLY "export API_KEY='my-personal-key'"
# NOT expected: DB_URL (local config replaces shared entirely)

# Verify DB_URL is NOT in the output
$DV export | grep DB_URL
# Expected: no output (exit code 1)
```

## Scenario 4: `dotvault add` — interactive mode

```bash
cd /tmp/dotvault-test && mkdir s4 && cd s4
$DV init

# Run interactive add
$DV add
# Interactive prompts:
#   Env var name: DATABASE_URL
#   Provider: (select "env" from list)
#   Reference (path/URI): MY_DB_URL
# Expected: "✓ Added DATABASE_URL to .dotvault.toml"

cat .dotvault.toml
# Expected: DATABASE_URL entry under [secrets]

# Add another one non-interactively
$DV add --name REDIS_URL --provider env --ref MY_REDIS

cat .dotvault.toml
# Expected: both DATABASE_URL and REDIS_URL under [secrets]
# Original comments from init should be preserved
```

## Scenario 5: `dotvault add --local`

```bash
cd /tmp/dotvault-test && mkdir s5 && cd s5
$DV init

# Add to local config instead
$DV add --local --name SECRET_KEY --provider env --ref MY_SECRET

ls -la .dotvault*
# Expected: both .dotvault.toml and .dotvault.local.toml exist

cat .dotvault.local.toml
# Expected: SECRET_KEY entry

# Local should take over
export MY_SECRET="local-secret"
$DV export
# Expected: export SECRET_KEY='local-secret'
```

## Scenario 6: macOS Keychain provider — set and resolve

Uses the real macOS Keychain. No external tools needed.

```bash
cd /tmp/dotvault-test && mkdir s6 && cd s6

# Store a secret in the keychain
$DV put --provider keychain --ref test-api-key --value "keychain-secret-value"
# Expected: "✓ Stored secret at test-api-key via keychain"
# Note: macOS may show a keychain access prompt — allow it

# Create a config that reads from keychain
cat > .dotvault.toml << 'EOF'
[secrets]
API_KEY = { provider = "keychain", ref = "test-api-key" }
EOF

# Resolve it
$DV export
# Expected: export API_KEY='keychain-secret-value'

$DV run -- sh -c 'echo $API_KEY'
# Expected: keychain-secret-value

# Store another secret interactively
$DV put
# Interactive prompts:
#   Provider: (select "keychain")
#   Reference: test-db-password
#   Value (hidden): ****  (type: my-db-pass)
# Expected: "✓ Stored secret at test-db-password via keychain"

# Verify round-trip
$DV add --name DB_PASS --provider keychain --ref test-db-password
$DV run -- sh -c 'echo $DB_PASS'
# Expected: my-db-pass
```

## Scenario 7: HashiCorp Vault — set and resolve (Docker)

Spins up a real Vault dev server.

```bash
# Start Vault
docker run -d --name dotvault-vault \
  -p 8200:8200 \
  -e VAULT_DEV_ROOT_TOKEN_ID=test-root-token \
  --cap-add IPC_LOCK \
  hashicorp/vault server -dev -dev-listen-address=0.0.0.0:8200

# Wait for it
sleep 2
curl -s http://localhost:8200/v1/sys/health | grep initialized
# Expected: "initialized":true

cd /tmp/dotvault-test && mkdir s7 && cd s7

# Store a secret via dotvault set
$DV put \
  --provider hashicorp \
  --ref "secret/data/myapp" \
  --field "api_key" \
  --value "vault-stored-secret-123"
# Note: needs VAULT_TOKEN or config. Set it:
export VAULT_TOKEN=test-root-token

# Retry with token set
$DV put \
  --provider hashicorp \
  --ref "secret/data/myapp" \
  --field "api_key" \
  --value "vault-stored-secret-123"
# Expected: "✓ Stored secret at secret/data/myapp via hashicorp"

# Verify it's actually in Vault
curl -s -H "X-Vault-Token: test-root-token" \
  http://localhost:8200/v1/secret/data/myapp | python3 -m json.tool
# Expected: {"data": {"data": {"api_key": "vault-stored-secret-123"}, ...}}

# Now resolve it via dotvault
cat > .dotvault.toml << 'EOF'
[providers.hashicorp]
address = "http://localhost:8200"
token = "test-root-token"

[secrets]
API_KEY = { provider = "hashicorp", ref = "secret/data/myapp", field = "api_key" }
EOF

$DV export
# Expected: export API_KEY='vault-stored-secret-123'

$DV run -- sh -c 'echo "Vault secret: $API_KEY"'
# Expected: Vault secret: vault-stored-secret-123

# Store a second field
$DV put \
  --provider hashicorp \
  --ref "secret/data/myapp" \
  --field "db_password" \
  --value "super-secret-db-pass"

# Add it to config and resolve both
$DV add --name DB_PASS --provider hashicorp --ref "secret/data/myapp" --field db_password

$DV run -- sh -c 'echo "$API_KEY | $DB_PASS"'
# Expected: vault-stored-secret-123 | super-secret-db-pass

# Cleanup
docker stop dotvault-vault && docker rm dotvault-vault
```

## Scenario 8: AWS Secrets Manager + SSM (LocalStack Docker)

```bash
# Start LocalStack
docker run -d --name dotvault-localstack \
  -p 4566:4566 \
  -e SERVICES=secretsmanager,ssm \
  -e DEFAULT_REGION=us-east-1 \
  localstack/localstack

sleep 5
curl -s http://localhost:4566/_localstack/health | grep running
# Expected: "running" status

cd /tmp/dotvault-test && mkdir s8 && cd s8

# Set AWS credentials for LocalStack (any values work)
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_DEFAULT_REGION=us-east-1

# Store a secret in Secrets Manager via dotvault set
$DV put \
  --provider aws \
  --ref "sm://myapp/openai-key" \
  --value "sk-aws-stored-key-456"
# Note: needs endpoint_url config for LocalStack

# Create config with LocalStack endpoint
cat > .dotvault.toml << 'EOF'
[providers.aws]
region = "us-east-1"
endpoint_url = "http://localhost:4566"

[secrets]
OPENAI_KEY = { provider = "aws", ref = "sm://myapp/openai-key" }
EOF

# Seed the secret directly via AWS CLI (since set needs the config first)
aws --endpoint-url=http://localhost:4566 secretsmanager create-secret \
  --name myapp/openai-key \
  --secret-string "sk-aws-stored-key-456" \
  --region us-east-1 2>/dev/null || \
aws --endpoint-url=http://localhost:4566 secretsmanager put-secret-value \
  --secret-id myapp/openai-key \
  --secret-string "sk-aws-stored-key-456" \
  --region us-east-1

# Resolve it
$DV export
# Expected: export OPENAI_KEY='sk-aws-stored-key-456'

# Also test SSM Parameter Store
aws --endpoint-url=http://localhost:4566 ssm put-parameter \
  --name "/myapp/db-url" \
  --value "postgres://prod:5432/mydb" \
  --type SecureString \
  --region us-east-1 \
  --overwrite 2>/dev/null

$DV add --name DB_URL --provider aws --ref "ssm:///myapp/db-url"

$DV run -- sh -c 'echo "$OPENAI_KEY | $DB_URL"'
# Expected: sk-aws-stored-key-456 | postgres://prod:5432/mydb

# Test JSON field extraction from Secrets Manager
aws --endpoint-url=http://localhost:4566 secretsmanager create-secret \
  --name myapp/multi-secret \
  --secret-string '{"user":"admin","pass":"s3cret"}' \
  --region us-east-1

$DV add --name DB_PASS --provider aws --ref "sm://myapp/multi-secret" --field pass
$DV run -- sh -c 'echo $DB_PASS'
# Expected: s3cret

# Cleanup
docker stop dotvault-localstack && docker rm dotvault-localstack
```

## Scenario 9: age-encrypted file provider

No external services needed — pure crypto.

```bash
cd /tmp/dotvault-test && mkdir s9 && cd s9

# Install age if not present
which age || brew install age

# Generate a keypair
age-keygen -o key.txt 2>pubkey.txt
cat pubkey.txt
# Expected: Public key: age1...

# Encrypt a secret
echo -n "age-encrypted-secret-789" | age -r $(grep 'age1' pubkey.txt | head -1) -o secret.age

# Create config
cat > .dotvault.toml << 'EOF'
[providers.age]
identity = "./key.txt"

[secrets]
SECRET_TOKEN = { provider = "age", ref = "./secret.age" }
EOF

# Resolve
$DV export
# Expected: export SECRET_TOKEN='age-encrypted-secret-789'

$DV run -- sh -c 'echo $SECRET_TOKEN'
# Expected: age-encrypted-secret-789
```

## Scenario 10: Shell hook (zsh)

Tests the direnv-like auto-loading experience.

```bash
# Print the hook
$DV hook --shell zsh
# Expected: shell function + add-zsh-hook setup

# To actually test, add it to a temporary zsh session:
cd /tmp/dotvault-test && mkdir s10 && cd s10

cat > .dotvault.toml << 'EOF'
[secrets]
HOOK_TEST = { provider = "env", ref = "HOOK_SOURCE" }
EOF

export HOOK_SOURCE="auto-loaded-value"

# Source the hook and simulate cd
eval "$($DV hook --shell zsh)"

# The hook should have run on eval (initial load)
echo $HOOK_TEST
# Expected: auto-loaded-value
```

## Scenario 11: Error handling

```bash
cd /tmp/dotvault-test && mkdir s11 && cd s11

# No config file
$DV export 2>&1
# Expected: error message about missing config file, exit code 1

# Unknown provider
cat > .dotvault.toml << 'EOF'
[secrets]
KEY = { provider = "nonexistent", ref = "something" }
EOF

$DV export 2>&1
# Expected: error about unknown provider

# Missing env var
cat > .dotvault.toml << 'EOF'
[secrets]
KEY = { provider = "env", ref = "THIS_VAR_DOES_NOT_EXIST_EVER" }
EOF

unset THIS_VAR_DOES_NOT_EXIST_EVER
$DV export 2>&1
# Expected: error about resolving KEY

# Init when file already exists
$DV init 2>&1
# Expected: error about .dotvault.toml already exists

# Set with unsupported write provider
$DV put --provider env --ref something --value test 2>&1
# Expected: error about provider not supporting writing
```

## Scenario 12: The full team workflow

Simulates the VP R&D + developer workflow end-to-end.

```bash
# --- VP R&D's machine ---
cd /tmp/dotvault-test && mkdir s12-team && cd s12-team

# VP creates the project config
$DV init

# VP adds mappings for the team secrets
$DV add --name OPENAI_API_KEY --provider keychain --ref team-openai-key
$DV add --name DATABASE_URL --provider env --ref SHARED_DB_URL

# VP stores the actual OpenAI key in the keychain
$DV put --provider keychain --ref team-openai-key --value "sk-proj-team-key-from-vp"

# VP commits .dotvault.toml (simulated: just show it)
cat .dotvault.toml
# VP tells team: "pull latest, run `dotvault run -- <your command>`"

# --- Developer's machine ---
# Developer clones repo (simulated: we're in the same dir)
export SHARED_DB_URL="postgres://dev:5432/localdb"

# Developer resolves all secrets
$DV export
# Expected:
#   export DATABASE_URL='postgres://dev:5432/localdb'
#   export OPENAI_API_KEY='sk-proj-team-key-from-vp'

$DV run -- sh -c 'echo "DB: $DATABASE_URL, AI: $OPENAI_API_KEY"'
# Expected: DB: postgres://dev:5432/localdb, AI: sk-proj-team-key-from-vp

# Developer wants to use their own OpenAI key for experiments
$DV add --local --name OPENAI_API_KEY --provider env --ref MY_PERSONAL_OPENAI
export MY_PERSONAL_OPENAI="sk-personal-dev-key"

$DV export
# Expected: export OPENAI_API_KEY='sk-personal-dev-key'
# (local config completely replaced shared — DATABASE_URL is gone)

# Developer adds DATABASE_URL to local too
$DV add --local --name DATABASE_URL --provider env --ref SHARED_DB_URL

$DV export
# Expected: both OPENAI_API_KEY and DATABASE_URL

# --- VP rotates the key ---
$DV put --provider keychain --ref team-openai-key --value "sk-proj-NEW-rotated-key"

# --- Developer without local override gets new key automatically ---
# (remove local override to simulate)
rm .dotvault.local.toml

$DV export
# Expected: export OPENAI_API_KEY='sk-proj-NEW-rotated-key'
# The rotation "just works" — no Slack message needed
```

---

## Cleanup

When done with all scenarios:

```bash
# Remove test directories
ls /tmp/dotvault-test/
# Manually remove what you don't need

# Remove test keychain entries (if created in scenario 6/12)
# Via macOS Keychain Access app, search for "dotvault" entries
```
