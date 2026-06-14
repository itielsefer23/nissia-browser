#!/usr/bin/env bash
#
# pre-push hook: auto version bump + tag based on conventional commits.
#
# Scans commits since the last tag:
#   feat: → minor bump
#   fix:  → patch bump
#   docs:/chore:/refactor:/test: → no bump
#
# Bumps workspace version in Cargo.toml, commits, and tags.
# The push then includes the bump commit + tag, triggering the Release workflow.

set -euo pipefail

CARGO_TOML="Cargo.toml"

# Get the latest tag
LAST_TAG=$(git tag --sort=-v:refname | head -1)
if [ -z "$LAST_TAG" ]; then
  echo "[version-bump] No tags found, skipping."
  exit 0
fi

# Get commits since last tag
COMMITS=$(git log "${LAST_TAG}..HEAD" --pretty=format:"%s" 2>/dev/null)
if [ -z "$COMMITS" ]; then
  exit 0
fi

# Check if the latest commit is already a version bump (avoid double-bump on re-push)
LATEST_COMMIT=$(git log -1 --pretty=format:"%s")
if echo "$LATEST_COMMIT" | grep -q "^chore: bump version to"; then
  exit 0
fi

# Determine bump type from commits AFTER the last bump (not all commits since tag)
BUMP=""
FOUND_BUMP=false
while IFS= read -r msg; do
  if echo "$msg" | grep -q "^chore: bump version to"; then
    FOUND_BUMP=true
    break
  fi
  case "$msg" in
    feat:*|feat\(*) BUMP="minor"; break ;;
    fix:*|fix\(*)   [ -z "$BUMP" ] && BUMP="patch" ;;
  esac
done <<< "$COMMITS"

if [ -z "$BUMP" ]; then
  exit 0
fi

# Parse current version
CURRENT=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# Calculate new version
if [ "$BUMP" = "minor" ]; then
  MINOR=$((MINOR + 1))
  PATCH=0
else
  PATCH=$((PATCH + 1))
fi
NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"

echo "[version-bump] ${CURRENT} → ${NEW_VERSION} (${BUMP})"

# Update Cargo.toml
sed -i '' "s/^version = \"${CURRENT}\"/version = \"${NEW_VERSION}\"/" "$CARGO_TOML"

# Update Cargo.lock
cargo check --quiet 2>/dev/null || true

# Commit and tag
git add "$CARGO_TOML" Cargo.lock
git commit --quiet -m "chore: bump version to ${NEW_VERSION}"
git tag "v${NEW_VERSION}"

echo "[version-bump] Tagged v${NEW_VERSION}"
