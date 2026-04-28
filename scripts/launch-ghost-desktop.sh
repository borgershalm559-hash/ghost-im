#!/usr/bin/env bash
# Launch a Ghost desktop instance with isolated identity storage.
# Usage:  ./scripts/launch-ghost-desktop.sh <profile-name>
# Example:  ./scripts/launch-ghost-desktop.sh alice
#
# Each profile gets its own GHOST_HOME directory. The first launch goes through
# onboarding; subsequent launches reuse the existing identity.

set -euo pipefail

PROFILE="${1:?profile name required (e.g. alice, bob)}"
PROFILE_DIR="$(pwd)/.tmp/profiles/${PROFILE}"
mkdir -p "${PROFILE_DIR}"

export GHOST_HOME="${PROFILE_DIR}"
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"

echo "GHOST_HOME=${GHOST_HOME}"
echo "Launching Ghost desktop (profile: ${PROFILE})"

cd apps/ghost-desktop
exec cargo +1.87-x86_64-pc-windows-msvc tauri dev
