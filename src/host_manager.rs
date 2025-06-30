use std::fs;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf; 

#[derive(Debug, Clone, PartialEq, Eq,Serialize, Deserialize)]
pub struct HostEntry {
    pub(crate) ip: String,
    pub(crate) hostname: String,
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq ,Serialize,Deserialize)]
pub enum Line {
    Comment(String),
    Entry(HostEntry),
    Empty,
}

pub fn load_hosts_entries() -> Vec<Line> {
    let contents = fs::read_to_string(get_hosts_file_path()).unwrap_or_else(|e| {
        println!("Errore nella lettura del file hosts: {}", e);
        String::new()
    });
    contents.lines()
        .map(|line| {
            let trimmed_line = line.trim();
            if trimmed_line.is_empty() {
                Line::Empty
            } else if trimmed_line.starts_with('#') {
                Line::Comment(line.to_string())
            } else {
                let parts: Vec<&str> = line.splitn(2, '#').collect();
                let record_part = parts[0].trim();
                let comment_part = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };

                let record_fields: Vec<&str> = record_part.split_whitespace().collect();

                if record_fields.len() >= 2 {
                    Line::Entry(HostEntry {
                        ip: record_fields[0].to_string(),
                        hostname: record_fields[1].to_string(),
                        comment: comment_part,
                    })
                } else {
                    Line::Comment(line.to_string())
                }
            }
        })
        .collect()
}

pub fn write_hosts_entries_to_file(entries: &[Line]) -> std::io::Result<()> {
    // 1. Crea il record localhost che deve essere sempre presente
    let localhost_entry = Line::Entry(HostEntry {
        ip: "127.0.0.1".to_string(),
        hostname: "localhost".to_string(),
        comment: None,
    });

    // 2. Controlla se il record localhost è già nella lista
    let mut updated_entries: Vec<Line> = entries.to_vec();
    if !updated_entries.contains(&localhost_entry) {
        // Se non è presente, aggiungilo all'inizio della lista per coerenza
        updated_entries.insert(0, localhost_entry);
    }

    // 3. Costruisci il contenuto da scrivere nel file
    let content: String = updated_entries
        .iter()
        .map(|line| match line {
            Line::Entry(entry) => format!("{} {}\n", entry.ip, entry.hostname),
            Line::Comment(comment) => format!("{}\n", comment),
            Line::Empty => "\n".to_string(),
        })
        .collect();
    
    // 4. Scrivi il contenuto aggiornato nel file
    fs::write(get_hosts_file_path(), content.as_bytes())?;

    Ok(())
}
// Ritorna il percorso corretto del file hosts in base al sistema operativo
fn get_hosts_file_path() -> PathBuf {
    let os = env::consts::OS;

    match os {
        "windows" => {
            // Su Windows, il percorso è C:\Windows\System32\drivers\etc\hosts
            let mut path = PathBuf::from(env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()));
            path.push("System32");
            path.push("drivers");
            path.push("etc");
            path.push("hosts");
            path
        },
        // Linux, macOS e altri sistemi Unix-like usano /etc/hosts
        _ => PathBuf::from("/etc/hosts"),
    }
}