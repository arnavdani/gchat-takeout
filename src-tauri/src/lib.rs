pub mod db;
pub mod models;
pub mod parser;

use tauri_plugin_dialog::DialogExt;
use tauri::{Manager, Emitter, menu::{Menu, MenuItem, Submenu, PredefinedMenuItem}};
use rusqlite::Connection;
use serde::Serialize;
use chrono::{Utc, TimeZone, NaiveDateTime};
use chrono_tz::US::Pacific;
use std::path::Path;
use std::fs;

#[derive(Serialize)]
struct GroupSummary {
    id: i64,
    google_id: String,
    name: Option<String>,
    group_type: String,
    message_count: i64,
    last_message_at: Option<String>,
}

#[derive(Serialize)]
struct AttachmentEntry {
    id: i64,
    original_name: String,
    export_name: String,
    local_path: String,
}

#[derive(Serialize)]
struct MessageEntry {
    id: i64,
    user_name: String,
    user_email: Option<String>,
    text: Option<String>,
    created_at: String,
    topic_id: Option<String>,
    attachments: Vec<AttachmentEntry>,
    is_me: bool,
}

fn format_to_pst(iso_str: &str) -> String {
    let fmt = "%Y-%m-%d %H:%M:%S";
    if let Ok(naive) = NaiveDateTime::parse_from_str(iso_str, fmt) {
        let utc_dt = Utc.from_utc_datetime(&naive);
        let pst_dt = utc_dt.with_timezone(&Pacific);
        return pst_dt.format("%m/%d/%Y %H:%M").to_string();
    }
    iso_str.to_string()
}

fn get_db_conn(app: &tauri::AppHandle) -> Result<Connection, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_dir.join("chat_logs.db");
    Connection::open(db_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_config(app: tauri::AppHandle, key: String) -> Result<Option<String>, String> {
    let conn = get_db_conn(&app)?;
    let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1").map_err(|e| e.to_string())?;
    let mut rows = stmt.query([key]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(row.get(0).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn set_config(app: tauri::AppHandle, key: String, value: String) -> Result<(), String> {
    let conn = get_db_conn(&app)?;
    conn.execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_groups(app: tauri::AppHandle, query: Option<String>) -> Result<Vec<GroupSummary>, String> {
    let conn = get_db_conn(&app)?;
    let q = query.unwrap_or_default().to_lowercase();
    let sql = if q.is_empty() {
        "SELECT g.id, g.google_id, COALESCE(g.name, (SELECT u.name FROM users u JOIN group_memberships gm ON u.id = gm.user_id WHERE gm.group_id = g.id AND u.is_main_user = 0 LIMIT 1)) as resolved_name, g.type, (SELECT COUNT(*) FROM messages WHERE group_id = g.id) as msg_count, g.last_message_at FROM groups g ORDER BY g.last_message_at DESC NULLS LAST".to_string()
    } else {
        format!("SELECT g.id, g.google_id, COALESCE(g.name, (SELECT u.name FROM users u JOIN group_memberships gm ON u.id = gm.user_id WHERE gm.group_id = g.id AND u.is_main_user = 0 LIMIT 1)) as resolved_name, g.type, (SELECT COUNT(*) FROM messages WHERE group_id = g.id) as msg_count, g.last_message_at FROM groups g WHERE g.id IN (SELECT group_id FROM group_memberships gm JOIN users u ON gm.user_id = u.id WHERE u.name LIKE '%{}%' OR u.email LIKE '%{}%') OR g.name LIKE '%{}%' ORDER BY g.last_message_at DESC NULLS LAST", q, q, q)
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        let last_msg_raw: Option<String> = row.get(5)?;
        Ok(GroupSummary { id: row.get(0)?, google_id: row.get(1)?, name: row.get(2)?, group_type: row.get(3)?, message_count: row.get(4)?, last_message_at: last_msg_raw.map(|s| format_to_pst(&s)), })
    }).map_err(|e| e.to_string())?;
    let mut groups = Vec::new();
    for row in rows { groups.push(row.map_err(|e| e.to_string())?); }
    Ok(groups)
}

#[tauri::command]
async fn get_messages(app: tauri::AppHandle, group_id: i64, limit: i64, offset: i64, query: Option<String>) -> Result<Vec<MessageEntry>, String> {
    let conn = get_db_conn(&app)?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let media_dir = app_dir.join("media");
    let takeout_path: String = conn.query_row("SELECT value FROM config WHERE key = 'takeout_path'", [], |r| r.get(0)).unwrap_or_default();
    let google_id: String = conn.query_row("SELECT google_id FROM groups WHERE id = ?1", [group_id], |r| r.get(0)).unwrap_or_default();
    let q = query.unwrap_or_default().to_lowercase();
    let (sql, params_vec) = if q.is_empty() {
        ("SELECT m.id, u.name, u.email, m.text, m.created_at, m.topic_id, u.is_main_user FROM messages m JOIN users u ON m.user_id = u.id WHERE m.group_id = ?1 ORDER BY m.created_at DESC LIMIT ?2 OFFSET ?3", vec![group_id.to_string(), limit.to_string(), offset.to_string()])
    } else {
        ("SELECT m.id, u.name, u.email, m.text, m.created_at, m.topic_id, u.is_main_user FROM messages m JOIN users u ON m.user_id = u.id WHERE m.group_id = ?1 AND m.text LIKE ?2 ORDER BY m.created_at DESC LIMIT ?3 OFFSET ?4", vec![group_id.to_string(), format!("%{}%", q), limit.to_string(), offset.to_string()])
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
        let msg_id: i64 = row.get(0)?;
        let created_at_raw: String = row.get(4)?;
        let is_me: i32 = row.get(6)?;
        let mut att_stmt = conn.prepare("SELECT id, original_name, export_name, is_copied FROM attachments WHERE message_id = ?1").unwrap();
        let att_rows = att_stmt.query_map([msg_id], |att_row| {
            let export_name: String = att_row.get(2)?;
            let is_copied: i32 = att_row.get(3)?;
            let managed_path = media_dir.join(group_id.to_string()).join(&export_name);
            let final_path = if is_copied == 1 && managed_path.exists() { managed_path } else { Path::new(&takeout_path).join("Groups").join(format!("{} {}", if google_id.len() > 11 { "Space" } else { "DM" }, google_id)).join(&export_name) };
            Ok(AttachmentEntry { id: att_row.get(0)?, original_name: att_row.get(1)?, export_name, local_path: final_path.to_string_lossy().into_owned(), })
        }).unwrap();
        let mut attachments = Vec::new();
        for att in att_rows { attachments.push(att.unwrap()); }
        Ok(MessageEntry { id: msg_id, user_name: row.get(1)?, user_email: row.get(2)?, text: row.get(3)?, created_at: format_to_pst(&created_at_raw), topic_id: row.get(5)?, attachments, is_me: is_me == 1, })
    }).map_err(|e| e.to_string())?;
    let mut messages = Vec::new();
    for row in rows { messages.push(row.map_err(|e| e.to_string())?); }
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move { let _ = sync_media_for_group(app_clone, group_id).await; });
    Ok(messages)
}

async fn sync_media_for_group(app: tauri::AppHandle, group_id: i64) -> Result<(), String> {
    let conn = get_db_conn(&app)?;
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let media_dir = app_dir.join("media");
    let takeout_path: String = conn.query_row("SELECT value FROM config WHERE key = 'takeout_path'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let google_id: String = conn.query_row("SELECT google_id FROM groups WHERE id = ?1", [group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let g_type: String = conn.query_row("SELECT type FROM groups WHERE id = ?1", [group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, export_name FROM attachments WHERE group_id = ?1 AND is_copied = 0").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([group_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    let source_dir = Path::new(&takeout_path).join("Groups").join(format!("{} {}", g_type, google_id));
    let dest_dir = media_dir.join(group_id.to_string());
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    for row in rows {
        if let Ok((id, export_name)) = row {
            let src = source_dir.join(&export_name);
            let dest = dest_dir.join(&export_name);
            if src.exists() { if let Ok(_) = fs::copy(src, dest) { let _ = conn.execute("UPDATE attachments SET is_copied = 1 WHERE id = ?1", [id]); } }
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_group_members(app: tauri::AppHandle, group_id: i64) -> Result<Vec<String>, String> {
    let conn = get_db_conn(&app)?;
    let mut stmt = conn.prepare("SELECT u.name FROM users u JOIN group_memberships gm ON u.id = gm.user_id WHERE gm.group_id = ?1 ORDER BY u.name ASC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([group_id], |row| row.get(0)).map_err(|e| e.to_string())?;
    let mut members = Vec::new();
    for row in rows { members.push(row.map_err(|e| e.to_string())?); }
    Ok(members)
}

#[tauri::command]
async fn import_takeout(app: tauri::AppHandle) -> Result<String, String> {
    let folder = app.dialog().file().blocking_pick_folder();
    if let Some(folder_path) = folder {
        let path = folder_path.into_path().map_err(|_| "Failed to resolve folder path".to_string())?;
        let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        if !app_dir.exists() { fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?; }
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
        .setup(|app| {
            let settings_item = MenuItem::with_id(app, "show-settings", "Settings...", true, Some("CmdOrCtrl+,"))?;
            let app_menu = Submenu::with_items(app, "GChat Takeout", true, &[
                &PredefinedMenuItem::about(app, None, None)?,
                &PredefinedMenuItem::separator(app)?,
                &settings_item,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, None)?,
            ])?;
            let edit_menu = Submenu::with_items(app, "Edit", true, &[
                &PredefinedMenuItem::undo(app, None)?,
                &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ])?;
            let menu = Menu::with_items(app, &[&app_menu, &edit_menu])?;
            app.set_menu(menu)?;
            app.on_menu_event(move |app, event| {
                if event.id() == "show-settings" {
                    let _ = app.emit("show-settings", ());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_takeout, get_groups, get_messages, get_group_members, get_config, set_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
