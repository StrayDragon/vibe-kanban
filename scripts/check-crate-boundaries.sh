#!/usr/bin/env bash
set -euo pipefail

# Workspace crate boundary checks.
#
# Intended to be run in CI to prevent accidental dependency regressions:
# - Protocol crates must not depend on web/MCP frameworks (Axum/rmcp).
# - Core crates must not depend on web/MCP frameworks (Axum/rmcp).
# - Persistence crates (db) must not depend on executor runtime crates.

cd "$(dirname "$0")/.."

if command -v rg >/dev/null 2>&1; then
  GREP_Q=(rg -q)
  GREP_N=(rg -n)
else
  GREP_Q=(grep -qE)
  GREP_N=(grep -nE)
fi

failures=0

check_no_dep() {
  local crate="$1"
  local pattern="$2"
  local rule="$3"

  if cargo tree -p "$crate" | "${GREP_Q[@]}" "$pattern"; then
    echo "❌ Boundary violation: $rule"
    echo "   crate: $crate"
    echo "   matched: $pattern"
    cargo tree -p "$crate" | "${GREP_N[@]}" "$pattern" | head -n 50
    echo
    failures=1
  else
    echo "✅ $rule ($crate)"
  fi
}

framework_pattern='\\b(axum|rmcp) v'
executor_core_pattern='\\bexecutors-core v'
executor_provider_pattern='\\bexecutor-[^ ]+ v'

protocol_crates=(
  executors-protocol
  logs-protocol
)

core_crates=(
  db
  services
  logs-store
)

for crate in "${protocol_crates[@]}"; do
  check_no_dep "$crate" "$framework_pattern" "protocol crate must not depend on Axum/rmcp"
  check_no_dep "$crate" "$executor_core_pattern" "protocol crate must not depend on executors-core runtime"
  check_no_dep "$crate" "$executor_provider_pattern" "protocol crate must not depend on executor provider crates"
done

for crate in "${core_crates[@]}"; do
  check_no_dep "$crate" "$framework_pattern" "core crate must not depend on Axum/rmcp"
done

check_no_dep "db" '\\bexecutors v' "db must not depend on executors runtime"
check_no_dep "db" "$executor_core_pattern" "db must not depend on executors-core runtime"
check_no_dep "db" "$executor_provider_pattern" "db must not depend on executor provider crates"

exit "$failures"
