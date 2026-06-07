#!/usr/bin/env bash
# Static codebase-intelligence checks: complexity, duplication, circular deps, arch boundaries.
# Mirrors .github/workflows/quality.yml so findings reproduce locally.
#
# Usage:
#   scripts/quality.sh        # hard gates fail the script; complexity/graph are reporting only
#
# Tooling (install once):
#   cargo install cargo-modules cargo-machete --locked
#   npx jscpd  (Node >= 18; fetched on demand)
set -uo pipefail
cd "$(dirname "$0")/.."

status=0
note() { printf '\n\033[1;34m== %s ==\033[0m\n' "$1"; }

# --- Hard gates ---
note "Format check"
cargo fmt --all --check || status=1

note "Lints (clippy correctness)"
cargo clippy --all-targets -- -D warnings || status=1

note "Unused dependencies (cargo-machete)"
cargo machete || status=1

note "Code duplication (jscpd, fails over 5% threshold)"
npx --yes jscpd || status=1

# --- Reporting only ---
note "Complexity hotspots (clippy — informational)"
cargo clippy --all-targets -- \
  -W clippy::cognitive_complexity \
  -W clippy::too_many_lines \
  -W clippy::too_many_arguments 2>&1 \
  | grep -E "complexity|too many lines|too many arguments|-->" || true

note "Module dependency graph (circular deps / arch boundaries — informational)"
echo "Acyclic check (note: cargo-modules flags benign type/method self-refs):"
cargo modules dependencies --lib --no-externs --acyclic -p agent-bridge-mcp || true
echo "Module-level graph (review for boundary violations):"
cargo modules dependencies --lib --no-externs --no-fns --no-types --no-traits --no-owns -p agent-bridge-mcp || true

exit "$status"
