#!/usr/bin/env bash
set -euo pipefail

# Workspace crate boundary checks.
#
# Intended to be run in CI to prevent accidental dependency regressions:
# - Protocol crates must not depend on web/MCP frameworks (Axum/rmcp).
# - Capability/foundation crates must not depend on web/MCP frameworks (Axum/rmcp).
# - Persistence crates (db) must not depend on executor runtime crates.
# - Executor registry/providers must stay isolated from server/runtime/domain crates.
# - Source-level imports must respect the planned crate boundaries.

cd "$(dirname "$0")/.."

if command -v rg >/dev/null 2>&1; then
  GREP_Q=(rg -q)
  GREP_N=(rg -n)
else
  GREP_Q=(grep -qE)
  GREP_N=(grep -nE)
fi

failures=0

check_no_source_match() {
  local pattern="$1"
  local rule="$2"
  shift 2

  local tmp_file
  tmp_file="$(mktemp)"

  if command -v rg >/dev/null 2>&1; then
    if rg -n "$pattern" "$@" >"$tmp_file"; then
      echo "❌ Boundary violation: $rule"
      cat "$tmp_file"
      echo
      failures=1
    else
      echo "✅ $rule"
    fi
  else
    if grep -nRE "$pattern" "$@" >"$tmp_file"; then
      echo "❌ Boundary violation: $rule"
      cat "$tmp_file"
      echo
      failures=1
    else
      echo "✅ $rule"
    fi
  fi

  rm -f "$tmp_file"
}

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
  logs-store
  repos
  tasks
  execution
  config
  events
)

executor_provider_crates=(
  executor-amp
  executor-claude
  executor-codex
  executor-copilot
  executor-cursor
  executor-droid
  executor-fake-agent
  executor-gemini
  executor-opencode
  executor-qwen
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

domain_runtime_pattern='\\b(server|app-runtime|repos|tasks|execution|config|events) v'

check_no_dep "executors" "$domain_runtime_pattern" "executors must not depend on server/runtime/domain crates"

for crate in "${executor_provider_crates[@]}"; do
  check_no_dep "$crate" "$domain_runtime_pattern" "executor provider must not depend on server/runtime/domain crates"
done

check_no_source_match \
  'use (axum|rmcp)|axum::|rmcp::' \
  'db/logs/capability crates must not import Axum/rmcp directly' \
  crates/db crates/logs-store crates/repos crates/tasks crates/execution crates/config crates/events

check_no_source_match \
  'executors_core::|executor_[a-z0-9_]+::' \
  'server must not import executor providers or executors_core directly' \
  crates/server/src

check_no_source_match \
  '\brepos::git::GitCli\b|\bGitCli::new\b' \
  'server/runtime/domain crates must not use GitCli directly' \
  crates/server crates/app-runtime crates/tasks crates/config crates/events

check_no_source_match \
  'git2::|Repository::open|WalkBuilder|notify::' \
  'blocking repo/filesystem primitives must stay inside repos or execution' \
  crates/server crates/app-runtime crates/tasks crates/config crates/events

exit "$failures"
