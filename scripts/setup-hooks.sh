#!/usr/bin/env bash
# Install git hooks for the nissia repository.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
HOOKS_DIR="${REPO_ROOT}/.git/hooks"

ln -sf ../../scripts/pre-push-version-bump.sh "${HOOKS_DIR}/pre-push"

echo "Git hooks installed:"
echo "  pre-push → auto version bump + tag (feat: minor, fix: patch)"
