# Tauri Plugin SQLite - JavaScript Bindings (@bspeckco/tauri-plugin-sqlite)

This package provides the JavaScript/TypeScript bindings for the `@bspeckco/tauri-plugin-sqlite` Tauri plugin.

**Note:** This is a fork of the official `tauri-plugin-sql`. It uses `rusqlite` instead of `sqlx`, **supporting only SQLite databases**, and adds explicit transaction support.

## Installation

You need to install both the Rust Core plugin and these JavaScript bindings.

See the [main plugin README](../../README.md) for instructions on setting up the Rust Core plugin in your `Cargo.toml`.

Install the JavaScript bindings using your preferred package manager:

```bash
# Using pnpm
pnpm add @bspeckco/tauri-plugin-sqlite

# Using npm
npm install @bspeckco/tauri-plugin-sqlite

# Using yarn
yarn add @bspeckco/tauri-plugin-sqlite
```

## Usage

Import the `Database` class and use the `load` method to connect to your SQLite database.

```typescript
import Database from '@bspeckco/tauri-plugin-sqlite';

async function initializeDb() {
  try {
    // Load a file-based database (relative to AppData dir)
    const db = await Database.load('sqlite:my-app-data.db');

    // Or load an in-memory database
    // const db = await Database.load('sqlite::memory:');

    console.log('Database loaded successfully');

    // Example query
    await db.execute(
      'CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)'
    );
    const result = await db.execute('INSERT INTO users (name) VALUES (?)', [
      'Test User',
    ]);
    console.log('Insert result:', result);

    const users = await db.select('SELECT * FROM users');
    console.log('Selected users:', users);

    // See the main plugin README for transaction examples
  } catch (error) {
    console.error('Failed to initialize database:', error);
  }
}

initializeDb();
```

Refer to the [main plugin README](../../README.md) for detailed API documentation, including transaction usage. 