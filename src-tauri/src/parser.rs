use std::fs;
use std::path::Path;
use rusqlite::{params, Connection, Result};
use walkdir::WalkDir;
use serde::Deserialize;
use chrono::{Utc, NaiveDateTime, TimeZone};
use crate::models::{GoogleMessagesFile, GoogleGroupInfo};
use crate::db::{upsert_user, upsert_group};

#[derive(Deserialize)]
struct UserInfoFile {
    user: UserInfoUser,
}

#[derive(Deserialize)]
struct UserInfoUser {
    email: String,
}

fn parse_google_date(date_str: &str) -> Option<String> {
    let clean = date_str.replace(" ", " ").replace(" at ", " ");
    let date_part = if let Some(comma_pos) = clean.find(", ") {
        &clean[comma_pos + 2..]
    } else {
        &clean
    };
    let date_no_tz = if date_part.ends_with(" UTC") {
        &date_part[..date_part.len() - 4]
    } else {
        date_part
    };

    let fmts = ["%B %d, %Y %I:%M:%S %p", "%B %e, %Y %I:%M:%S %p"];
    for fmt in fmts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(date_no_tz.trim(), fmt) {
            return Some(Utc.from_utc_datetime(&naive).format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }
    None
}

pub fn process_takeout_dir(takeout_path: &Path, conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    // Store takeout path in config
    conn.execute(
        "INSERT INTO config (key, value) VALUES ('takeout_path', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![takeout_path.to_string_lossy()],
    )?;

    let mut main_user_email: Option<String> = None;
    let users_path = takeout_path.join("Users");
    if users_path.exists() {
        for entry in WalkDir::new(users_path).max_depth(2) {
            let entry = entry?;
            if entry.file_name() == "user_info.json" {
                let content = fs::read_to_string(entry.path())?;
                if let Ok(info) = serde_json::from_str::<UserInfoFile>(&content) {
                    main_user_email = Some(info.user.email);
                    break;
                }
            }
        }
    }

    let groups_path = takeout_path.join("Groups");
    if !groups_path.exists() {
        return Err("Groups directory not found in takeout path".into());
    }

    let tx = conn.transaction()?;

    for entry in WalkDir::new(groups_path).min_depth(1).max_depth(1) {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            continue;
        }

        let dir_path = entry.path();
        let dir_name = entry.file_name().to_string_lossy();
        
        let (group_type, google_id) = if dir_name.starts_with("DM ") {
            ("DM", dir_name.trim_start_matches("DM ").to_string())
        } else if dir_name.starts_with("Space ") {
            ("Space", dir_name.trim_start_matches("Space ").to_string())
        } else {
            ("Unknown", dir_name.to_string())
        };

        let group_info_path = dir_path.join("group_info.json");
        let mut group_name: Option<String> = None;
        let mut members: Vec<crate::models::GoogleMember> = Vec::new();

        if group_info_path.exists() {
            let info_content = fs::read_to_string(&group_info_path)?;
            if let Ok(info) = serde_json::from_str::<GoogleGroupInfo>(&info_content) {
                group_name = info.name;
                members = info.members;
            }
        }
        
        let group_db_id = upsert_group(&tx, &google_id, group_name.as_deref(), group_type)?;

        for member in members {
            let is_main = main_user_email.as_deref() == member.email.as_deref();
            let user_db_id = upsert_user(&tx, &member.name, member.email.as_deref(), &member.user_type, is_main)?;
            tx.execute(
                "INSERT OR IGNORE INTO group_memberships (user_id, group_id) VALUES (?1, ?2)",
                params![user_db_id, group_db_id],
            )?;
        }

        let messages_path = dir_path.join("messages.json");
        if messages_path.exists() {
            let messages_content = fs::read_to_string(&messages_path)?;
            let messages_file: GoogleMessagesFile = serde_json::from_str(&messages_content)?;
            let mut latest_msg_date: Option<String> = None;

            for msg in messages_file.messages {
                let is_main = main_user_email.as_deref() == msg.creator.email.as_deref();
                let user_db_id = upsert_user(&tx, &msg.creator.name, msg.creator.email.as_deref(), &msg.creator.user_type, is_main)?;
                
                let iso_date = parse_google_date(&msg.created_date).unwrap_or_else(|| msg.created_date.clone());
                
                if latest_msg_date.is_none() || iso_date > *latest_msg_date.as_ref().unwrap() {
                    latest_msg_date = Some(iso_date.clone());
                }

                tx.execute(
                    "INSERT INTO messages (group_id, user_id, text, created_at, topic_id, google_message_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(google_message_id) DO UPDATE SET
                        text = excluded.text,
                        created_at = excluded.created_at",
                    params![group_db_id, user_db_id, msg.text, iso_date, msg.topic_id, msg.message_id],
                )?;

                let message_db_id: i64 = tx.query_row(
                    "SELECT id FROM messages WHERE google_message_id = ?1",
                    params![msg.message_id],
                    |row| row.get(0),
                )?;

                if let Some(attachments) = msg.attached_files {
                    for att in attachments {
                        // Metadata only - no copying here!
                        tx.execute(
                            "INSERT INTO attachments (message_id, group_id, original_name, export_name)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![message_db_id, group_db_id, att.original_name, att.export_name],
                        )?;
                    }
                }
            }

            if let Some(last_date) = latest_msg_date {
                tx.execute("UPDATE groups SET last_message_at = ?1 WHERE id = ?2", params![last_date, group_db_id])?;
            }
        }
    }

    tx.commit()?;
    Ok(())
}
