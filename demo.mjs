#!/usr/bin/env node

import { execSync, spawnSync } from "child_process";
import { mkdirSync, writeFileSync, existsSync, readFileSync, rmSync } from "fs";
import { join } from "path";
import { createInterface } from "readline";

const DV = join(import.meta.dirname, "target/release/dv");
const BASE = "/tmp/dotvault-demo";

const rl = createInterface({ input: process.stdin, output: process.stdout });
const ask = (q) => new Promise((r) => rl.question(q, r));

const RESET = "\x1b[0m";
const BOLD = "\x1b[1m";
const DIM = "\x1b[2m";
const GREEN = "\x1b[32m";
const CYAN = "\x1b[36m";
const YELLOW = "\x1b[33m";
const RED = "\x1b[31m";
const MAGENTA = "\x1b[35m";

let currentDir = process.cwd();
let extraEnv = {};

function header(text) {
  console.log();
  console.log(`${BOLD}${MAGENTA}${"═".repeat(60)}${RESET}`);
  console.log(`${BOLD}${MAGENTA}  ${text}${RESET}`);
  console.log(`${BOLD}${MAGENTA}${"═".repeat(60)}${RESET}`);
  console.log();
}

function narrate(text) {
  console.log(`${DIM}${text}${RESET}`);
}

function showCmd(cmd) {
  console.log(`${BOLD}${CYAN}  $ ${cmd}${RESET}`);
}

function showExpected(text) {
  console.log(`${YELLOW}  Expected: ${text}${RESET}`);
}

function showResult(stdout, stderr, status) {
  if (stdout.trim()) {
    for (const line of stdout.trim().split("\n")) {
      console.log(`${GREEN}  ${line}${RESET}`);
    }
  }
  if (stderr.trim()) {
    for (const line of stderr.trim().split("\n")) {
      console.log(`${RED}  ${line}${RESET}`);
    }
  }
  if (status !== 0 && status !== null) {
    console.log(`${DIM}  (exit code: ${status})${RESET}`);
  }
}

function run(cmd, { expectFail = false, env: envOverrides = {} } = {}) {
  showCmd(cmd);
  const fullEnv = { ...process.env, ...extraEnv, ...envOverrides };
  const result = spawnSync("sh", ["-c", cmd], {
    cwd: currentDir,
    env: fullEnv,
    encoding: "utf-8",
    timeout: 30000,
  });
  showResult(result.stdout || "", result.stderr || "", result.status);
  return result;
}

function writeFile(name, content) {
  const p = join(currentDir, name);
  writeFileSync(p, content);
  showCmd(`cat > ${name}`);
  for (const line of content.trim().split("\n")) {
    console.log(`${DIM}  ${line}${RESET}`);
  }
}

function setEnv(key, value) {
  extraEnv[key] = value;
  showCmd(`export ${key}="${value}"`);
}

function cd(dir) {
  currentDir = dir;
  showCmd(`cd ${dir}`);
}

function setupDir(name) {
  const dir = join(BASE, name);
  if (existsSync(dir)) {
    rmSync(dir, { recursive: true });
  }
  mkdirSync(dir, { recursive: true });
  cd(dir);
}

async function waitForUser(scenarioNext) {
  console.log();
  const label = scenarioNext
    ? `${DIM}Press Enter to continue to ${scenarioNext}, or 'q' to quit...${RESET}`
    : `${DIM}Press Enter to continue, or 'q' to quit...${RESET}`;
  const answer = await ask(label);
  if (answer.trim().toLowerCase() === "q") {
    console.log("\nBye!");
    process.exit(0);
  }
}

// ─────────────────────────────────────────────

async function scenario1() {
  header("Scenario 1: Basic env provider — the happy path");
  narrate("Tests the core flow: init, add a mapping, export, run.");
  setupDir("s1");

  await waitForUser();

  narrate("Step 1: Init a new project");
  run(`${DV} init`);
  showExpected('Created .dotvault.toml');

  await waitForUser();

  narrate("Step 2: Show the starter template");
  run(`cat .dotvault.toml`);

  await waitForUser();

  narrate('Step 3: Add a secret mapping (non-interactive)');
  narrate('VP said: "I set up MY_OPENAI_KEY, map it as OPENAI_API_KEY"');
  run(`${DV} add --name OPENAI_API_KEY --provider env --ref MY_OPENAI_KEY`);

  await waitForUser();

  narrate("Step 4: Verify the config was updated");
  run(`cat .dotvault.toml`);
  showExpected("OPENAI_API_KEY entry under [secrets]");

  await waitForUser();

  narrate("Step 5: Set the env var and export");
  setEnv("MY_OPENAI_KEY", "sk-test-12345");
  run(`${DV} export`);
  showExpected("export OPENAI_API_KEY='sk-test-12345'");

  await waitForUser();

  narrate("Step 6: Run a command with injected secrets");
  run(`${DV} run -- sh -c 'echo "My key is: $OPENAI_API_KEY"'`);
  showExpected("My key is: sk-test-12345");
}

async function scenario2() {
  header("Scenario 2: Multiple secrets from env provider");
  setupDir("s2");

  writeFile(".dotvault.toml", `[secrets]
DB_URL = { provider = "env", ref = "TEST_DB_URL" }
API_KEY = { provider = "env", ref = "TEST_API_KEY" }
REDIS_URL = { provider = "env", ref = "TEST_REDIS" }
`);

  setEnv("TEST_DB_URL", "postgres://localhost:5432/mydb");
  setEnv("TEST_API_KEY", "key-abc-123");
  setEnv("TEST_REDIS", "redis://localhost:6379");

  await waitForUser();

  narrate("Export all three secrets");
  run(`${DV} export`);
  showExpected("Three export lines");

  await waitForUser();

  narrate("Run a command that uses all three");
  run(`${DV} run -- sh -c 'echo "$DB_URL | $API_KEY | $REDIS_URL"'`);
  showExpected("postgres://localhost:5432/mydb | key-abc-123 | redis://localhost:6379");
}

async function scenario3() {
  header("Scenario 3: Local config replaces shared");
  narrate("Simulates a developer overriding team config with personal settings.");
  setupDir("s3");

  narrate("Team config (checked in):");
  writeFile(".dotvault.toml", `[secrets]
API_KEY = { provider = "env", ref = "TEAM_KEY" }
DB_URL = { provider = "env", ref = "TEAM_DB" }
`);

  narrate("\nDeveloper's local override (gitignored):");
  writeFile(".dotvault.local.toml", `[secrets]
API_KEY = { provider = "env", ref = "MY_KEY" }
`);

  setEnv("TEAM_KEY", "team-value");
  setEnv("TEAM_DB", "team-db-url");
  setEnv("MY_KEY", "my-personal-key");

  await waitForUser();

  narrate("Local config completely REPLACES shared — no merge");
  run(`${DV} export`);
  showExpected("ONLY API_KEY='my-personal-key' — NO DB_URL");
}

async function scenario4() {
  header("Scenario 4: dotvault add — interactive mode");
  narrate("This scenario requires interactive input. Using non-interactive flags to simulate.");
  setupDir("s4");

  run(`${DV} init`);

  await waitForUser();

  narrate('Add first secret: DATABASE_URL pointing to env var MY_DB_URL');
  run(`${DV} add --name DATABASE_URL --provider env --ref MY_DB_URL`);

  await waitForUser();

  narrate("Add second secret: REDIS_URL");
  run(`${DV} add --name REDIS_URL --provider env --ref MY_REDIS`);

  await waitForUser();

  narrate("Config should have both entries, and original comments preserved:");
  run(`cat .dotvault.toml`);
}

async function scenario5() {
  header("Scenario 5: dotvault add --local");
  setupDir("s5");

  run(`${DV} init`);

  await waitForUser();

  narrate("Add a secret to the local config instead of shared");
  run(`${DV} add --local --name SECRET_KEY --provider env --ref MY_SECRET`);

  await waitForUser();

  narrate("Both files should exist:");
  run("ls -la .dotvault*");

  await waitForUser();

  narrate("Local config contents:");
  run("cat .dotvault.local.toml");

  await waitForUser();

  narrate("Local takes over — only local secrets resolved");
  setEnv("MY_SECRET", "local-secret");
  run(`${DV} export`);
  showExpected("export SECRET_KEY='local-secret'");
}

async function scenario6() {
  header("Scenario 6: macOS Keychain — put and resolve");
  narrate("Uses the real macOS Keychain. May prompt for access.");
  setupDir("s6");

  await waitForUser();

  narrate("Store a secret in the keychain via dotvault put");
  run(`${DV} put --provider keychain --ref dotvault-demo-api-key --value "keychain-secret-value"`);
  showExpected("✓ Stored secret at dotvault-demo-api-key via keychain");

  await waitForUser();

  narrate("Create a config that reads from keychain");
  writeFile(".dotvault.toml", `[secrets]
API_KEY = { provider = "keychain", ref = "dotvault-demo-api-key" }
`);

  await waitForUser();

  narrate("Resolve it");
  run(`${DV} export`);
  showExpected("export API_KEY='keychain-secret-value'");

  await waitForUser();

  narrate("Run a command with the keychain secret");
  run(`${DV} run -- sh -c 'echo "Keychain secret: $API_KEY"'`);
  showExpected("Keychain secret: keychain-secret-value");
}

async function scenario7() {
  header("Scenario 7: HashiCorp Vault (Docker)");
  narrate("Spins up a real Vault dev server in Docker.");

  await waitForUser();

  narrate("Starting Vault dev server...");
  run("docker rm -f dotvault-vault 2>/dev/null; docker run -d --name dotvault-vault -p 8200:8200 -e VAULT_DEV_ROOT_TOKEN_ID=test-root-token --cap-add IPC_LOCK hashicorp/vault server -dev -dev-listen-address=0.0.0.0:8200");

  narrate("Waiting for Vault to be ready...");
  run("sleep 3");
  run("curl -s http://localhost:8200/v1/sys/health | python3 -m json.tool | head -5");

  setupDir("s7");
  setEnv("VAULT_TOKEN", "test-root-token");

  await waitForUser();

  narrate("Store a secret via dotvault put");
  run(`${DV} put --provider hashicorp --ref "secret/data/myapp" --field "api_key" --value "vault-stored-secret-123"`);
  showExpected("✓ Stored secret via hashicorp");

  await waitForUser();

  narrate("Verify it's actually in Vault via curl");
  run(`curl -s -H "X-Vault-Token: test-root-token" http://localhost:8200/v1/secret/data/myapp | python3 -m json.tool | grep api_key`);

  await waitForUser();

  narrate("Create config and resolve via dotvault");
  writeFile(".dotvault.toml", `[providers.hashicorp]
address = "http://localhost:8200"
token = "test-root-token"

[secrets]
API_KEY = { provider = "hashicorp", ref = "secret/data/myapp", field = "api_key" }
`);

  run(`${DV} export`);
  showExpected("export API_KEY='vault-stored-secret-123'");

  await waitForUser();

  narrate("Store a second field and resolve both");
  run(`${DV} put --provider hashicorp --ref "secret/data/myapp" --field "db_password" --value "super-secret-db-pass"`);
  run(`${DV} add --name DB_PASS --provider hashicorp --ref "secret/data/myapp" --field db_password`);
  run(`${DV} run -- sh -c 'echo "$API_KEY | $DB_PASS"'`);
  showExpected("vault-stored-secret-123 | super-secret-db-pass");

  await waitForUser();

  narrate("Stopping Vault...");
  run("docker stop dotvault-vault && docker rm dotvault-vault");
}

async function scenario8() {
  header("Scenario 8: Error handling");
  setupDir("s8");

  narrate("No config file:");
  run(`${DV} export`, { expectFail: true });
  showExpected("Error about missing config file");

  await waitForUser();

  narrate("Unknown provider:");
  writeFile(".dotvault.toml", `[secrets]
KEY = { provider = "nonexistent", ref = "something" }
`);
  run(`${DV} export`, { expectFail: true });
  showExpected("Error about unknown provider");

  await waitForUser();

  narrate("Missing env var:");
  writeFile(".dotvault.toml", `[secrets]
KEY = { provider = "env", ref = "THIS_VAR_DOES_NOT_EXIST_EVER" }
`);
  delete extraEnv["THIS_VAR_DOES_NOT_EXIST_EVER"];
  run(`${DV} export`, { expectFail: true });
  showExpected("Error about resolving KEY");

  await waitForUser();

  narrate("Init when file already exists:");
  run(`${DV} init`, { expectFail: true });
  showExpected("Error about .dotvault.toml already exists");

  await waitForUser();

  narrate("Put with unsupported write provider:");
  run(`${DV} put --provider env --ref something --value test`, { expectFail: true });
  showExpected("Error about provider not supporting writing");
}

async function scenario9() {
  header("Scenario 9: The full team workflow");
  narrate("VP R&D creates project → stores secrets → developer uses them → VP rotates → dev gets new value.");
  setupDir("s9");

  narrate("\n--- VP R&D's machine ---\n");

  narrate("VP creates the project config");
  run(`${DV} init`);

  await waitForUser();

  narrate("VP adds mappings for team secrets");
  run(`${DV} add --name OPENAI_API_KEY --provider keychain --ref dotvault-demo-team-key`);
  run(`${DV} add --name DATABASE_URL --provider env --ref SHARED_DB_URL`);

  await waitForUser();

  narrate("VP stores the actual OpenAI key in the keychain");
  run(`${DV} put --provider keychain --ref dotvault-demo-team-key --value "sk-proj-team-key-from-vp"`);

  await waitForUser();

  narrate("VP shows the config (this gets committed):");
  run("cat .dotvault.toml");

  await waitForUser();

  narrate("\n--- Developer's machine ---\n");
  narrate("Developer clones repo and sets their local DB URL");
  setEnv("SHARED_DB_URL", "postgres://dev:5432/localdb");

  narrate("Developer resolves all secrets:");
  run(`${DV} export`);
  showExpected("DATABASE_URL and OPENAI_API_KEY");

  await waitForUser();

  run(`${DV} run -- sh -c 'echo "DB: $DATABASE_URL, AI: $OPENAI_API_KEY"'`);

  await waitForUser();

  narrate("Developer wants their own OpenAI key for experiments");
  run(`${DV} add --local --name OPENAI_API_KEY --provider env --ref MY_PERSONAL_OPENAI`);
  run(`${DV} add --local --name DATABASE_URL --provider env --ref SHARED_DB_URL`);
  setEnv("MY_PERSONAL_OPENAI", "sk-personal-dev-key");

  run(`${DV} export`);
  showExpected("Personal key + shared DB");

  await waitForUser();

  narrate("\n--- VP rotates the key ---\n");
  run(`${DV} put --provider keychain --ref dotvault-demo-team-key --value "sk-proj-NEW-rotated-key"`);

  await waitForUser();

  narrate("Developer removes local override → gets rotated key automatically");
  const localPath = join(currentDir, ".dotvault.local.toml");
  if (existsSync(localPath)) {
    rmSync(localPath);
    showCmd("rm .dotvault.local.toml");
  }

  run(`${DV} export`);
  showExpected("OPENAI_API_KEY='sk-proj-NEW-rotated-key' — rotation just works!");
}

// ─────────────────────────────────────────────

async function main() {
  console.log();
  console.log(`${BOLD}${MAGENTA}  dotvault interactive demo${RESET}`);
  console.log(`${DIM}  Binary: ${DV}${RESET}`);
  console.log(`${DIM}  Test dir: ${BASE}${RESET}`);
  console.log();

  if (!existsSync(DV)) {
    console.log(`${RED}Release binary not found. Run: cargo build --release -p dotvault${RESET}`);
    process.exit(1);
  }

  mkdirSync(BASE, { recursive: true });

  const scenarios = [
    ["Scenario 1", "Basic env provider", scenario1],
    ["Scenario 2", "Multiple secrets", scenario2],
    ["Scenario 3", "Local replaces shared", scenario3],
    ["Scenario 4", "dotvault add", scenario4],
    ["Scenario 5", "dotvault add --local", scenario5],
    ["Scenario 6", "macOS Keychain", scenario6],
    ["Scenario 7", "HashiCorp Vault (Docker)", scenario7],
    ["Scenario 8", "Error handling", scenario8],
    ["Scenario 9", "Full team workflow", scenario9],
  ];

  console.log(`${BOLD}Scenarios:${RESET}`);
  for (const [i, [name, desc]] of scenarios.entries()) {
    console.log(`  ${i + 1}. ${name}: ${desc}`);
  }
  console.log(`  a. Run all`);
  console.log();

  const choice = await ask(`${BOLD}Which scenario? (1-${scenarios.length}, or 'a' for all): ${RESET}`);

  if (choice.trim().toLowerCase() === "a") {
    for (let i = 0; i < scenarios.length; i++) {
      const [name, desc, fn] = scenarios[i];
      await fn();
      if (i < scenarios.length - 1) {
        await waitForUser(scenarios[i + 1][0]);
      }
    }
  } else {
    const idx = parseInt(choice) - 1;
    if (idx >= 0 && idx < scenarios.length) {
      await scenarios[idx][2]();
    } else {
      console.log("Invalid choice.");
    }
  }

  console.log();
  console.log(`${BOLD}${GREEN}Demo complete!${RESET}`);
  console.log(`${DIM}Test files in ${BASE} — remove when done.${RESET}`);
  rl.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
