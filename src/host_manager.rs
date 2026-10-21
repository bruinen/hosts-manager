use std::fs;
use std::io;
use std::net::IpAddr;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use crate::privileges;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub ip: String,
    pub hostname: String,
    // Hostname aggiuntivi sulla stessa riga (es: "127.0.0.1 a.local b.local")
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub comment: Option<String>,
}
fn default_true() -> bool {
    true
}

impl Entry {
    // Crea un record validando l'IP; `names` contiene uno o più hostname separati da spazi
    pub fn new(ip: &str, names: &str) -> Result<Entry, String> {
        let ip = ip.trim();
        if !is_valid_ip(ip) {
            return Err(format!("IP non valido: '{}'", ip));
        }

        let mut names = names.split_whitespace().map(str::to_string);
        let hostname = names.next().ok_or("L'hostname è obbligatorio.")?;
        let aliases: Vec<String> = names.collect();

        if let Some(bad) = std::iter::once(&hostname).chain(&aliases).find(|n| n.contains('#')) {
            return Err(format!("Hostname non valido: '{}'", bad));
        }

        Ok(Entry {
            ip: ip.to_string(),
            hostname,
            aliases,
            enabled: true,
            comment: None,
        })
    }

    pub fn localhost() -> Entry {
        Entry {
            ip: "127.0.0.1".to_string(),
            hostname: "localhost".to_string(),
            aliases: Vec::new(),
            enabled: true,
            comment: None,
        }
    }

    // Il record localhost di default, gestito automaticamente dall'app
    pub fn is_localhost(&self) -> bool {
        *self == Entry::localhost()
    }

    // Hostname e alias separati da spazi
    pub fn names(&self) -> String {
        std::iter::once(self.hostname.as_str())
            .chain(self.aliases.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Line {
    Entry(Entry),
    Comment(String),
    Empty,
}


pub fn load_hosts_entries() -> Vec<Line> {
    let contents = fs::read_to_string(get_hosts_file_path()).unwrap_or_else(|e| {
        eprintln!("Errore nella lettura del file hosts: {}", e);
        String::new()
    });
    contents.lines().map(parse_line).collect()
}

// Converte una riga del file hosts in una Line
pub fn parse_line(line: &str) -> Line {
    let trimmed_line = line.trim();
    if trimmed_line.is_empty() {
        return Line::Empty;
    }

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

    // 3. Parsa IP, Hostname ed eventuali alias dalla parte rimanente
    let host_parts: Vec<&str> = host_ip_hostname_part.split_whitespace().collect();

    if host_parts.len() >= 2 && is_valid_ip(host_parts[0]) {
        Line::Entry(Entry {
            ip: host_parts[0].to_string(),
            hostname: host_parts[1].to_string(),
            aliases: host_parts[2..].iter().map(|s| s.to_string()).collect(),
            enabled: is_enabled,
            comment: end_comment,
        })
    } else {
        // Se non è un record valido (es: "# Host Database"), lo trattiamo come Line::Comment
        Line::Comment(line.to_string())
    }
}

// Valida un indirizzo IPv4/IPv6, ignorando l'eventuale zone id (es: "fe80::1%lo0")
pub fn is_valid_ip(value: &str) -> bool {
    let without_zone = value.split('%').next().unwrap_or(value);
    without_zone.parse::<IpAddr>().is_ok()
}

// Converte una Line nella riga da scrivere nel file hosts (senza '\n')
pub fn format_line(line: &Line) -> String {
    match line {
        Line::Entry(entry) => {
            let mut s = String::new();
            if !entry.enabled {
                s.push_str("# "); // Prepend '#' if disabled
            }

            // Formatta IP, Hostname e alias
            s.push_str(&format!("{:<15} {}", entry.ip, entry.names()));

            // Aggiungi il commento a fine riga se presente
            if let Some(comment_text) = &entry.comment {
                s.push_str(" # "); // Add " #" before the comment
                s.push_str(comment_text);
            }
            s
        },
        Line::Comment(comment) => comment.clone(),
        Line::Empty => String::new(),
    }
}

// Genera il contenuto completo del file hosts.
// Il record localhost viene sempre aggiunto in testa (e rimosso dalle righe per evitare duplicati).
pub fn render_hosts_file(entries: &[Line]) -> String {
    let header = [Line::Empty, Line::Entry(Entry::localhost())];
    let body = entries
        .iter()
        .filter(|line| !matches!(line, Line::Entry(entry) if entry.is_localhost()));

    header
        .iter()
        .chain(body)
        .map(|line| format_line(line) + "\n")
        .collect()
}

pub fn write_hosts_entries_to_file(entries: &[Line]) -> io::Result<()> {
    let path = get_hosts_file_path();
    let content = render_hosts_file(entries);

    match write_atomically(&path, content.as_bytes()) {
        // Senza permessi diretti, su Unix prova tramite la regola sudoers
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied && cfg!(unix) => {
            privileges::write_hosts_with_sudo(&path, content.as_bytes())
        }
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => Err(io::Error::new(
            e.kind(),
            format!("Permesso negato su {}: avvia l'app con privilegi di amministratore.", path.display()),
        )),
        result => result,
    }
}

// Scrive su un file temporaneo nella stessa cartella e poi lo rinomina, così un'interruzione
// non lascia mai il file a metà. Se il file è un symlink o la cartella non è scrivibile
// (es: è stato reso scrivibile solo il file), scrive direttamente sul file.
fn write_atomically(path: &Path, content: &[u8]) -> io::Result<()> {
    let is_symlink = fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if is_symlink {
        return fs::write(path, content);
    }

    let tmp_path = path.with_extension("hosts-manager.tmp");
    match fs::write(&tmp_path, content) {
        Ok(()) => {
            // Mantieni i permessi del file originale
            if let Ok(metadata) = fs::metadata(path) {
                let _ = fs::set_permissions(&tmp_path, metadata.permissions());
            }
            fs::rename(&tmp_path, path).inspect_err(|_| {
                let _ = fs::remove_file(&tmp_path);
            })
        }
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => fs::write(path, content),
        Err(e) => Err(e),
    }
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


// Ritorna true se l'hostname (o uno degli alias) del record contiene la query (case-insensitive).
// Una query vuota (o di soli spazi) corrisponde a tutti i record.
pub fn hostname_matches(entry: &Entry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || std::iter::once(&entry.hostname)
            .chain(&entry.aliases)
            .any(|name| name.to_lowercase().contains(&query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hostname: &str) -> Entry {
        Entry::new("10.0.0.1", hostname).unwrap()
    }

    #[test]
    fn empty_query_matches_everything() {
        assert!(hostname_matches(&entry("server.local"), ""));
        assert!(hostname_matches(&entry("server.local"), "   "));
    }

    #[test]
    fn matches_substring_case_insensitive() {
        assert!(hostname_matches(&entry("Api.Example.com"), "example"));
        assert!(hostname_matches(&entry("api.example.com"), " API "));
        assert!(!hostname_matches(&entry("api.example.com"), "db"));
    }

    #[test]
    fn matches_aliases() {
        assert!(hostname_matches(&entry("a.local b.local"), "b.loc"));
    }

    #[test]
    fn new_entry_validates_input() {
        let e = Entry::new(" 10.0.0.1 ", "a.local  b.local").unwrap();
        assert_eq!(e.ip, "10.0.0.1");
        assert_eq!(e.hostname, "a.local");
        assert_eq!(e.aliases, vec!["b.local"]);
        assert_eq!(e.names(), "a.local b.local");

        assert!(Entry::new("10.0.0", "a.local").is_err());
        assert!(Entry::new("server", "a.local").is_err());
        assert!(Entry::new("10.0.0.1", "   ").is_err());
        assert!(Entry::new("10.0.0.1", "a#b").is_err());
        assert!(Entry::new("::1", "a.local").is_ok());
    }

    #[test]
    fn parses_and_preserves_aliases() {
        let line = parse_line("127.0.0.1 a.local b.local c.local # dev");
        let Line::Entry(e) = &line else { panic!("expected entry, got {:?}", line) };
        assert_eq!(e.hostname, "a.local");
        assert_eq!(e.aliases, vec!["b.local", "c.local"]);
        assert_eq!(e.comment.as_deref(), Some("dev"));
        assert_eq!(format_line(&line), "127.0.0.1       a.local b.local c.local # dev");
    }

    #[test]
    fn text_comments_are_not_entries() {
        assert_eq!(parse_line("# Host Database"), Line::Comment("# Host Database".to_string()));
        assert_eq!(parse_line("##"), Line::Comment("##".to_string()));
    }

    #[test]
    fn disabled_entries_are_entries() {
        let line = parse_line("# 10.0.0.5 db.local");
        let Line::Entry(e) = &line else { panic!("expected entry, got {:?}", line) };
        assert!(!e.enabled);
        assert_eq!(format_line(&line), "# 10.0.0.5        db.local");
    }

    #[test]
    fn accepts_ipv6_with_zone() {
        assert!(matches!(parse_line("fe80::1%lo0 localhost"), Line::Entry(_)));
        assert!(matches!(parse_line("::1 localhost"), Line::Entry(_)));
    }

    #[test]
    fn old_json_without_aliases_still_loads() {
        let json = r#"{"Entry":{"ip":"10.0.0.1","hostname":"x.local","enabled":true,"comment":null}}"#;
        let line: Line = serde_json::from_str(json).unwrap();
        let Line::Entry(e) = line else { panic!() };
        assert!(e.aliases.is_empty());
    }

    #[test]
    fn render_adds_localhost_once() {
        let lines = vec![
            Line::Comment("# header".to_string()),
            Line::Entry(Entry::localhost()),
            Line::Entry(entry("db.local")),
        ];
        assert_eq!(
            render_hosts_file(&lines),
            "\n127.0.0.1       localhost\n# header\n10.0.0.1        db.local\n"
        );
    }

    #[test]
    fn round_trip_preserves_lines() {
        let original = "# comment\n10.0.0.1        a.local b.local # note\n\n# 10.0.0.2        off.local\n";
        let parsed: Vec<Line> = original.lines().map(parse_line).collect();
        let rendered: String = parsed.iter().map(|l| format_line(l) + "\n").collect();
        assert_eq!(rendered, original);
    }

    #[test]
    fn atomic_write_replaces_file() {
        let dir = env::temp_dir().join(format!("hosts-manager-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hosts");
        fs::write(&path, "old").unwrap();

        write_atomically(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert!(!path.with_extension("hosts-manager.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
