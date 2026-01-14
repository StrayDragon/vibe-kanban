#!/usr/bin/env node

const { execSync } = require('child_process');
const fs = require('fs');
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

// Create temporary database file
const dbFile = path.join(backendDir, 'prepare_db.sqlite');
fs.writeFileSync(dbFile, '');

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
  // Clean up temporary file
  if (fs.existsSync(dbFile)) {
    fs.unlinkSync(dbFile);
  }
}
