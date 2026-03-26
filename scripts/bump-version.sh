#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

CURRENT=$(grep '^version' "$REPO_ROOT/crates/dotvault/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

BUMP="${1:-}"

case "$BUMP" in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
  "")
    echo "Current version: $CURRENT"
    echo "Usage: $0 <major|minor|patch>"
    exit 1
    ;;
  *)
    echo "Unknown bump type: $BUMP (use major, minor, or patch)"
    exit 1
    ;;
esac

VERSION="$MAJOR.$MINOR.$PATCH"
echo "Bumping $CURRENT -> $VERSION"

# Update dotvault crate
sed -i '' "s/^version = \"$CURRENT\"/version = \"$VERSION\"/" "$REPO_ROOT/crates/dotvault/Cargo.toml"
echo "  Updated crates/dotvault/Cargo.toml"

# Update secret-resolvers crate
sed -i '' "s/^version = \"$CURRENT\"/version = \"$VERSION\"/" "$REPO_ROOT/crates/secret-resolvers/Cargo.toml"
echo "  Updated crates/secret-resolvers/Cargo.toml"

# Update secret-resolvers dependency version in dotvault
sed -i '' "s/secret-resolvers = { version = \"$CURRENT\"/secret-resolvers = { version = \"$VERSION\"/" "$REPO_ROOT/crates/dotvault/Cargo.toml"
echo "  Updated secret-resolvers dependency in crates/dotvault/Cargo.toml"

# Update platform packages
for pkg in cli-darwin-arm64 cli-linux-x64 cli-linux-arm64; do
  PKGJSON="$REPO_ROOT/npm/$pkg/package.json"
  if [ -f "$PKGJSON" ]; then
    node -e "
      const fs = require('fs');
      const pkg = JSON.parse(fs.readFileSync('$PKGJSON', 'utf8'));
      pkg.version = '$VERSION';
      fs.writeFileSync('$PKGJSON', JSON.stringify(pkg, null, 2) + '\n');
    "
    echo "  Updated npm/$pkg"
  fi
done

# Update main package (version + optionalDependencies)
MAIN_PKG="$REPO_ROOT/npm/dotvault/package.json"
if [ -f "$MAIN_PKG" ]; then
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$MAIN_PKG', 'utf8'));
    pkg.version = '$VERSION';
    if (pkg.optionalDependencies) {
      for (const dep of Object.keys(pkg.optionalDependencies)) {
        pkg.optionalDependencies[dep] = '$VERSION';
      }
    }
    fs.writeFileSync('$MAIN_PKG', JSON.stringify(pkg, null, 2) + '\n');
  "
  echo "  Updated npm/dotvault (main package)"
fi

# Update Claude Code plugin version
for json in "$REPO_ROOT/.claude-plugin/plugin.json" "$REPO_ROOT/.claude-plugin/marketplace.json"; do
  if [ -f "$json" ]; then
    node -e "
      const fs = require('fs');
      const data = JSON.parse(fs.readFileSync('$json', 'utf8'));
      if (data.version) data.version = '$VERSION';
      if (data.plugins) data.plugins.forEach(p => p.version = '$VERSION');
      fs.writeFileSync('$json', JSON.stringify(data, null, 2) + '\n');
    "
  fi
done
echo "  Updated .claude-plugin manifests"

# Sync Cargo.lock
(cd "$REPO_ROOT" && cargo generate-lockfile 2>/dev/null) && echo "  Updated Cargo.lock" || true

echo "Done. All packages set to $VERSION"
