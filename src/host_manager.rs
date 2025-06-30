use std::fs;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;


#[derive(Debug, Clone, PartialEq, Eq,Serialize, Deserialize)]
pub struct Entry {
    pub ip: String,
    pub hostname: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub comment: Option<String>,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone,PartialEq, Eq, Serialize, Deserialize)]
pub enum Line {
    Entry(Entry),
    Comment(String),
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
            } else {
                let mut is_enabled = true;
                let mut current_parse_line = trimmed_line;

                // 1. Controlla se la riga inizia con '#' per determinare 'enabled'
                if current_parse_line.starts_with("#") {
                    is_enabled = false;
                    current_parse_line = current_parse_line.trim_start_matches('#').trim();
                }

                // 2. Separa la parte IP/Hostname dal commento a fine riga
                let mut parts = current_parse_line.splitn(2, '#');
                let host_ip_hostname_part = parts.next().unwrap_or("").trim();
                let end_comment: Option<String> = parts.next().map(|s| s.trim().to_string());

                // 3. Parsa IP e Hostname dalla parte rimanente
                let host_parts: Vec<&str> = host_ip_hostname_part.split_whitespace().collect();

                if host_parts.len() >= 2 {
                    Line::Entry(Entry {
                        ip: host_parts[0].to_string(),
                        hostname: host_parts[1].to_string(),
                        enabled: is_enabled,
                        comment: end_comment,
                    })
                } else {
                    // Se non è un record valido (anche se commentato), lo trattiamo come Line::Comment
                    Line::Comment(line.to_string())
                }
            }
        })
        .collect()
}

pub fn write_hosts_entries_to_file(entries: &[Line]) -> std::io::Result<()> {
    // Aggiungi sempre il record localhost
    let mut updated_entries = vec![
        Line::Comment("".to_string()), // Aggiungi una riga vuota per chiarezza
        Line::Entry(Entry {
            ip: "127.0.0.1".to_string(),
            hostname: "localhost".to_string(),
            enabled: true,
            comment: None, // Il localhost di default non ha un commento
        }),
    ];

    for line in entries {
        // Ignora il record localhost nel file originale per evitare duplicati
        if let Line::Entry(entry) = line {
            if entry.ip == "127.0.0.1" && entry.hostname == "localhost" {
                continue;
            }
        }
        updated_entries.push(line.clone());
    }

    let content: String = updated_entries
        .iter()
        .map(|line| match line {
            Line::Entry(entry) => {
                let mut s = String::new();
                if !entry.enabled {
                    s.push_str("# "); // Prepend '#' if disabled
                }

                // Formatta IP e Hostname
                s.push_str(&format!("{:<15} {}", entry.ip, entry.hostname));

                // Aggiungi il commento a fine riga se presente
                if let Some(comment_text) = &entry.comment {
                    s.push_str(" # "); // Add " #" before the comment
                    s.push_str(comment_text);
                }
                s.push_str("\n");
                s
            },
            Line::Comment(comment) => format!("{}\n", comment),
            Line::Empty => "\n".to_string(),
        })
        .collect();

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