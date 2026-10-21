use rusqlite::{Connection, Result, params};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::host_manager::Line;
use std::fmt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use rusqlite::ffi::Error;

// Nome del profilo creato al primo avvio dal file hosts di sistema (non eliminabile)
pub const DEFAULT_PROFILE_NAME: &str = "Default";

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static SCHEMA_READY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub hosts: Vec<Line>,
    pub is_active: bool,
}

impl Profile {
    pub fn is_default(&self) -> bool {
        self.name == DEFAULT_PROFILE_NAME
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

fn db_path() -> &'static PathBuf {
    DB_PATH.get_or_init(|| {
        let mut path = dirs::data_dir().unwrap_or_else(|| {
            eprintln!("Application data directory not found, using the local directory.");
            PathBuf::from(".")
        });
        path.push("hosts_manager");
        path.push("profiles.db");
        path
    })
}

// Cartella che contiene il database dei profili
pub fn data_dir() -> &'static std::path::Path {
    db_path().parent().unwrap_or(std::path::Path::new("."))
}

// Apre la connessione al database; cartella e tabella vengono create solo al primo utilizzo
pub fn initialize_db() -> Result<Connection> {
    let path = db_path();
    let first_use = !SCHEMA_READY.load(Ordering::Acquire);

    if first_use && let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }

    let conn = Connection::open(path)?;

    if first_use {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                hosts_json TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        SCHEMA_READY.store(true, Ordering::Release);
    }

    Ok(conn)
}

fn hosts_to_json(hosts: &[Line]) -> Result<String> {
    serde_json::to_string(hosts).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

// Crea un nuovo profilo (non attivo) e lo ritorna
pub fn create_profile(conn: &Connection, name: &str, hosts: &[Line]) -> Result<Profile> {
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        hosts: hosts.to_vec(),
        is_active: false,
    };

    conn.execute(
        "INSERT INTO profiles (id, name, hosts_json, is_active) VALUES (?1, ?2, ?3, 0)",
        params![profile.id, profile.name, hosts_to_json(&profile.hosts)?],
    )?;
    Ok(profile)
}

pub fn get_all_profiles(conn: &Connection) -> Result<Vec<Profile>> {
    let mut stmt = conn.prepare("SELECT id, name, hosts_json, is_active FROM profiles")?;
    let profiles_iter = stmt.query_map([], |row| {
        let hosts_json: String = row.get(2)?;
        let hosts: Vec<Line> = serde_json::from_str(&hosts_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;

        let is_active_int: i64 = row.get(3)?;

        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            hosts,
            is_active: is_active_int != 0,
        })
    })?;

    profiles_iter.collect()
}

pub fn set_active_profile(conn: &Connection, profile_id: &str) -> Result<()> {
    // Un'unica UPDATE atomica: non restano mai zero o due profili attivi
    conn.execute("UPDATE profiles SET is_active = (id = ?1)", params![profile_id])?;
    Ok(())
}

pub fn delete_profile(conn: &Connection, profile_id: &str) -> Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])?;
    Ok(())
}

pub fn update_profile(conn: &Connection, profile: &Profile) -> Result<()> {
    conn.execute(
        "UPDATE profiles SET hosts_json = ?1 WHERE id = ?2",
        params![hosts_to_json(&profile.hosts)?, profile.id],
    )?;
    Ok(())
}

// Importa un profilo esportato: riceve un nuovo id e non viene attivato
pub fn import_profile(conn: &Connection, profile: &Profile) -> Result<Profile> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM profiles WHERE name = ?1",
        params![&profile.name],
        |row| row.get(0),
    )?;

    if count > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            Error::new(1),
            Some(format!("A profile named '{}' already exists.", profile.name)),
        ));
    }

    create_profile(conn, &profile.name, &profile.hosts)
}
