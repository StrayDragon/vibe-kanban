#!/usr/bin/env bash
set -euo pipefail

node scripts/check-external-links.js
node scripts/check-doc-links.js

cd frontend
npm run lint
./../scripts/check-i18n.sh
npm run format:check
npx tsc --noEmit
npm run test
NODE_OPTIONS=--max-old-space-size=8192 npm run build
cd ..

cargo fmt --all -- --check
./scripts/check-crate-boundaries.sh
npm run generate-types:check
npm run generate-env-docs:check
npm run prepare-db:check
npm run remote:prepare-db:check
cargo test --workspace
cargo clippy --all --all-targets --all-features -- -D warnings

