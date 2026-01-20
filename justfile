set dotenv-load := true
set shell := ["bash", "-uc"]

target_dir := `cargo metadata --format-version=1 --no-deps | node -e 'const fs = require("fs"); const data = fs.readFileSync(0, "utf8"); process.stdout.write(JSON.parse(data).target_directory);'`

default:
    @just -l

install:
    pnpm i

db-prepare:
    pnpm run prepare-db

db-prepare-check:
    pnpm run prepare-db:check

frontend-build: install
    pnpm -C frontend build

backend-build: # db-prepare
    cargo build -p server --release
    cargo build -p server --release --bin mcp_task_server
    cargo build -p executors --release --bin fake-agent

mcp-build:
    cargo build -p server --release --bin mcp_task_server

build: frontend-build backend-build mcp-build

run host="0.0.0.0" port="3001": frontend-build backend-build
    HOST={{host}} PORT={{port}} {{target_dir}}/release/server

dev: db-prepare
    pnpm run dev

check:
    pnpm run check

lint:
    pnpm run lint
