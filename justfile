set dotenv-load := true
set shell := ["bash", "-uc"]

default:
    @just -l

install:
    pnpm i

db-prepare:
    pnpm run prepare-db

db-prepare-check:
    pnpm run prepare-db:check

frontend-build:
    pnpm -C frontend build

backend-build: # db-prepare
    cargo build -p server --release

build: frontend-build backend-build

run host="0.0.0.0" port="3001": frontend-build backend-build
    HOST={{host}} PORT={{port}} /var/tmp/vibe-kanban/cache/cargo-target/release/server

dev: db-prepare
    pnpm run dev

check:
    pnpm run check

lint:
    pnpm run lint
