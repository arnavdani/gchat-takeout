pub mod db;
pub mod models;
pub mod parser;

use tauri_plugin_dialog::DialogExt;
use tauri::Manager;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
struct GroupSummary {
    id: i64,
    google_id: String,
    name: Option<String>,
    group_type: String,
    message_count: i64,
}

#[derive(Serialize)]
struct MessageEntry {
    id: i64,
    user_name: String,
    user_email: Option<String>,
    text: Option<String>,
    created_at: String,
    topic_id: Option<String>,
}

fn get_db_conn(app: &tauri::AppHandle) -> Result<Connection, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_dir.join("chat_logs.db");
    Connection::open(db_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_groups(app: tauri::AppHandle) -> Result<Vec<GroupSummary>, String> {
    let conn = get_db_conn(&app)?;
    
    // Complex query to handle naming:
    // 1. If it's a Space and has a name, use it.
    // 2. If it's a DM, find the name of the member who is NOT the main user.
    let mut stmt = conn.prepare(
        "SELECT 
            g.id, 
            g.google_id, 
            COALESCE(g.name, (
                SELECT u.name 
                FROM users u
                JOIN group_memberships gm ON u.id = gm.user_id
                WHERE gm.group_id = g.id AND u.is_main_user = 0
                LIMIT 1
            )) as resolved_name,
            g.type, 
            COUNT(m.id) as msg_count 
         FROM groups g
         LEFT JOIN messages m ON g.id = m.group_id
         GROUP BY g.id
         ORDER BY msg_count DESC"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(GroupSummary {
            id: row.get(0)?,
            google_id: row.get(1)?,
            name: row.get(2)?,
            group_type: row.get(3)?,
            message_count: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut groups = Vec::new();
    for row in rows {
        groups.push(row.map_err(|e| e.to_string())?);
    }
    Ok(groups)
}

#[tauri::command]
async fn get_messages(app: tauri::AppHandle, group_id: i64, limit: i64, offset: i64) -> Result<Vec<MessageEntry>, String> {
    let conn = get_db_conn(&app)?;
    let mut stmt = conn.prepare(
        "SELECT m.id, u.name, u.email, m.text, m.created_at, m.topic_id
         FROM messages m
         JOIN users u ON m.user_id = u.id
         WHERE m.group_id = ?1
         ORDER BY m.created_at DESC
         LIMIT ?2 OFFSET ?3"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![group_id, limit, offset], |row| {
        Ok(MessageEntry {
            id: row.get(0)?,
            user_name: row.get(1)?,
            user_email: row.get(2)?,
            text: row.get(3)?,
            created_at: row.get(4)?,
            topic_id: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| e.to_string())?);
    }
    Ok(messages)
}

#[tauri::command]
async fn import_takeout(app: tauri::AppHandle) -> Result<String, String> {
    let folder = app.dialog().file().blocking_pick_folder();

    if let Some(folder_path) = folder {
        let path = folder_path.into_path().map_err(|_| "Failed to resolve folder path".to_string())?;
        let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        if !app_dir.exists() {
            std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        }
        let db_path = app_dir.join("chat_logs.db");
        let mut conn = db::init_db(&db_path).map_err(|e| e.to_string())?;
        parser::process_takeout_dir(&path, &mut conn).map_err(|e| e.to_string())?;
        Ok("Import successful".to_string())
    } else {
        Err("No folder selected".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            import_takeout,
            get_groups,
            get_messages
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
