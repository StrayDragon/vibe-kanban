set dotenv-load := true
set shell := ["bash", "-uc"]

target_dir := `cargo metadata --format-version=1 --no-deps | node -e 'const fs = require("fs"); const data = fs.readFileSync(0, "utf8"); process.stdout.write(JSON.parse(data).target_directory);'`

default:
    @just -l

install force="0":
    #!/usr/bin/env bash
    set -euo pipefail

    cache_dir="{{target_dir}}/just"
    stamp="${cache_dir}/pnpm-install.stamp"

    mkdir -p "$cache_dir"

    needs_install=0

    if [[ "{{force}}" == "1" || "{{force}}" == "true" ]]; then
        needs_install=1
    fi

    if [[ ! -d node_modules ]] || [[ ! -d frontend/node_modules ]]; then
        needs_install=1
    elif [[ ! -f "$stamp" ]]; then
        needs_install=1
    else
        for f in pnpm-lock.yaml package.json pnpm-workspace.yaml .npmrc .pnpmrc frontend/package.json
        do
            if [[ -f "$f" && "$f" -nt "$stamp" ]]; then
                needs_install=1
                break
            fi
        done
    fi

    if [[ $needs_install -eq 1 ]]; then
        pnpm i
        touch "$stamp"
    else
        echo "install: up to date"
    fi

db-prepare:
    pnpm run prepare-db

db-prepare-check:
    pnpm run prepare-db:check

frontend-build force="0": install
    #!/usr/bin/env bash
    set -euo pipefail

    cache_dir="{{target_dir}}/just"
    stamp="${cache_dir}/frontend-build.stamp"

    mkdir -p "$cache_dir"

    needs_build=0

    if [[ "{{force}}" == "1" || "{{force}}" == "true" ]]; then
        needs_build=1
    fi

    if [[ ! -d frontend/dist ]]; then
        needs_build=1
    elif [[ ! -f "$stamp" ]]; then
        needs_build=1
    else
        if find frontend \
            -path frontend/dist -prune -o \
            -path frontend/node_modules -prune -o \
            -type f -newer "$stamp" -print -quit | grep -q .; then
            needs_build=1
        elif find shared -type f -newer "$stamp" -print -quit 2>/dev/null | grep -q .; then
            needs_build=1
        else
            for f in pnpm-lock.yaml package.json pnpm-workspace.yaml .env
            do
                if [[ -f "$f" && "$f" -nt "$stamp" ]]; then
                    needs_build=1
                    break
                fi
            done
        fi
    fi

    if [[ $needs_build -eq 1 ]]; then
        pnpm -C frontend build
        touch "$stamp"
    else
        echo "frontend-build: up to date"
    fi

backend-build: # db-prepare
    cargo build -p server --release
    cargo build -p server --release --bin mcp_task_server
    cargo build -p executors --release --bin fake-agent

mcp-build:
    cargo build -p server --release --bin mcp_task_server

mcp-inspector: mcp-build
    npx @modelcontextprotocol/inspector -- {{target_dir}}/release/mcp_task_server

build: frontend-build backend-build mcp-build

run host="0.0.0.0" port="3001" open="0": frontend-build backend-build
    HOST={{host}} PORT={{port}} VK_OPEN_BROWSER_STARTUP={{open}} {{target_dir}}/release/server

run-force host="0.0.0.0" port="3001":
    just install 1
    just frontend-build 1
    just run {{host}} {{port}}

dev: db-prepare
    pnpm run dev

check:
    pnpm run check

lint:
    pnpm run lint
