use rusqlite::{params, Connection, Result};
use std::path::Path;

pub fn init_db<P: AsRef<Path>>(path: P) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Table for app settings (like takeout path)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY,
            value TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT,
            user_type TEXT,
            is_main_user INTEGER DEFAULT 0,
            UNIQUE(name, email)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            google_id TEXT UNIQUE NOT NULL,
            name TEXT,
            type TEXT,
            last_message_at DATETIME
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_groups_last_msg ON groups (last_message_at DESC)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS group_memberships (
            user_id INTEGER,
            group_id INTEGER,
            PRIMARY KEY (user_id, group_id),
            FOREIGN KEY(user_id) REFERENCES users(id),
            FOREIGN KEY(group_id) REFERENCES groups(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER,
            user_id INTEGER,
            text TEXT,
            created_at DATETIME,
            topic_id TEXT,
            google_message_id TEXT UNIQUE,
            FOREIGN KEY(group_id) REFERENCES groups(id),
            FOREIGN KEY(user_id) REFERENCES users(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_group_time ON messages (group_id, created_at)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER,
            group_id INTEGER,
            original_name TEXT,
            export_name TEXT,
            is_copied INTEGER DEFAULT 0,
            FOREIGN KEY(message_id) REFERENCES messages(id),
            FOREIGN KEY(group_id) REFERENCES groups(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_attachments_group ON attachments (group_id)",
        [],
    )?;

    Ok(conn)
}

pub fn upsert_user(conn: &Connection, name: &str, email: Option<&str>, user_type: &str, is_main: bool) -> Result<i64> {
    conn.execute(
        "INSERT INTO users (name, email, user_type, is_main_user)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(name, email) DO UPDATE SET
            user_type = excluded.user_type,
            is_main_user = CASE WHEN users.is_main_user = 1 THEN 1 ELSE excluded.is_main_user END",
        params![name, email, user_type, if is_main { 1 } else { 0 }],
    )?;
    
    let id: i64 = conn.query_row(
        "SELECT id FROM users WHERE name = ?1 AND (email = ?2 OR (email IS NULL AND ?2 IS NULL))",
        params![name, email],
        |row| row.get(0),
    )?;
    
    Ok(id)
}

pub fn upsert_group(conn: &Connection, google_id: &str, name: Option<&str>, group_type: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO groups (google_id, name, type)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(google_id) DO UPDATE SET
            name = COALESCE(excluded.name, groups.name),
            type = excluded.type",
        params![google_id, name, group_type],
    )?;

    let id: i64 = conn.query_row(
        "SELECT id FROM groups WHERE google_id = ?1",
        params![google_id],
        |row| row.get(0),
    )?;

    Ok(id)
}
