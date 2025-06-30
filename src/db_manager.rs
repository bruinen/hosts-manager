use rusqlite::{Connection, Result, params};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::host_manager::Line;
use std::fmt;
use std::path::PathBuf;
use dirs;
use rusqlite::ffi::Error;

// Struct per rappresentare un profilo nel database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub hosts: Vec<Line>,
    pub is_active: bool, // Nuovo campo per i dati degli host
}

// Implementazione del trait Display per la struttura Profile
impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// Inizializza il database e crea la tabella 'profiles' con il nuovo schema
pub fn initialize_db() -> Result<Connection> {
    
    // 1. Trova la directory dei dati dell'applicazione
    let mut db_path: PathBuf = dirs::data_dir().unwrap_or_else(|| {
        // Se non riesce a trovare la directory, usa la directory corrente
        eprintln!("Impossibile trovare la directory dei dati dell'applicazione, verrà usato il percorso locale.");
        PathBuf::from(".")
    });

    // 2. Aggiungi il nome della tua applicazione
    db_path.push("hosts_manager");

    // 3. Crea la directory se non esiste
    std::fs::create_dir_all(&db_path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // 4. Aggiungi il nome del file del database
    db_path.push("profiles.db");

    // 5. Apri la connessione al database
    let conn = Connection::open(&db_path)?;
   
    conn.execute(
        "CREATE TABLE IF NOT EXISTS profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            hosts_json TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    Ok(conn)
}

pub fn create_profile(conn: &Connection, name: &str, hosts: &[Line]) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let hosts_json = serde_json::to_string(hosts).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    conn.execute(
        "INSERT INTO profiles (id, name, hosts_json, is_active) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, hosts_json, 0], // Inizializza a 0 (false)
    )?;
    Ok(())
}

// Recupera tutti i profili dal database
pub fn get_all_profiles(conn: &Connection) -> Result<Vec<Profile>> {
    let mut stmt = conn.prepare("SELECT id, name, hosts_json, is_active FROM profiles")?;
    let profiles_iter = stmt.query_map([], |row| {
        let hosts_json: String = row.get(2)?;
        let hosts: Vec<Line> = serde_json::from_str(&hosts_json) // E qui deserializzi
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;

        let is_active_int: i64 = row.get(3)?;
        let is_active = is_active_int != 0;

        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            hosts,
            is_active,
        })
    })?;

    let profiles: Result<Vec<Profile>> = profiles_iter.collect();
    profiles
}
pub fn set_active_profile(conn: &Connection, profile_id: &str) -> Result<()> {
    // 1. Disattiva tutti i profili
    conn.execute("UPDATE profiles SET is_active = 0", [])?;

    // 2. Attiva il profilo specificato
    conn.execute("UPDATE profiles SET is_active = 1 WHERE id = ?1", params![profile_id])?;

    Ok(())
}
// Elimina un profilo dal database in base all'ID
pub fn delete_profile(conn: &Connection, profile_id: &str) -> Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])?;
    Ok(())
}

pub fn update_profile(conn: &Connection, profile: &Profile) -> Result<()> {
    let hosts_json = serde_json::to_string(&profile.hosts).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    conn.execute(
        "UPDATE profiles SET hosts_json = ?1 WHERE id = ?2",
        params![hosts_json, profile.id],
    )?;
    Ok(())
}

pub fn import_profile(conn: &Connection, profile: &Profile) -> Result<()> {
    // 1. Controlla se un profilo con lo stesso nome esiste già
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM profiles WHERE name = ?1")?;
    let count: i64 = stmt.query_row(params![&profile.name], |row| row.get(0))?;

    if count > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            Error::new(1),
            Some(format!("Un profilo con il nome '{}' esiste già.", profile.name)),
        ));
    }

    // 2. Inserisci il nuovo profilo nel database
    let hosts_json = serde_json::to_string(&profile.hosts)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    conn.execute(
        "INSERT INTO profiles (id, name, hosts_json, is_active) VALUES (?1, ?2, ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            &profile.name,
            hosts_json,
            profile.is_active as i32
        ],
    )?;

    Ok(())
}