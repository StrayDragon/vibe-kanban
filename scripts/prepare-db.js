#!/usr/bin/env node

const { execSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const checkMode = process.argv.includes('--check');

console.log(
  checkMode
    ? 'Checking SeaORM migrations...'
    : 'Preparing database for SeaORM migrations...'
);

// Change to database crate directory
const backendDir = path.join(__dirname, '..', 'crates/db');
process.chdir(backendDir);

// Create temporary database under OS temp to avoid polluting the repo worktree.
const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'vk-prepare-db-'));
const dbFile = path.join(tempRoot, 'prepare_db.sqlite');
fs.writeFileSync(dbFile, '');

let cleaned = false;
function cleanup() {
  if (cleaned) return;
  cleaned = true;

  try {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  } catch {
    // ignore
  }
}

process.on('SIGINT', () => {
  cleanup();
  process.exit(130);
});

process.on('SIGTERM', () => {
  cleanup();
  process.exit(143);
});

try {
  // Get absolute path (cross-platform)
  const dbPath = path.resolve(dbFile);
  const databaseUrl = `sqlite://${dbPath}`;

  console.log(`Using database: ${databaseUrl}`);

  // Run migrations
  console.log('Running SeaORM migrations...');
  execSync('cargo run -p db-migration -- up', {
    stdio: 'inherit',
    env: { ...process.env, DATABASE_URL: databaseUrl }
  });

  console.log(checkMode ? 'SeaORM migration check complete!' : 'Database preparation complete!');
} finally {
  cleanup();
}
