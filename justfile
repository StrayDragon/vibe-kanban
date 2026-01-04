set dotenv-load := true
set shell := ["bash", "-uc"]

default:
    @just -l

install:
    pnpm i

frontend-build:
    pnpm -C frontend build

backend-build:
    cargo build -p server --release

build: frontend-build backend-build

run host="0.0.0.0" port="3001": frontend-build backend-build
    HOST={{host}} PORT={{port}} ./target/release/server

dev:
    pnpm run dev

check:
    pnpm run check

lint:
    pnpm run lint
