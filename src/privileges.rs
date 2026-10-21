// Gestione dei permessi per scrivere il file hosts senza avviare l'app come root.
//
// L'app gira come utente normale. Se la scrittura diretta di /etc/hosts non è permessa,
// il contenuto viene passato a `sudo -n /usr/bin/tee /etc/hosts`. Per evitare la richiesta
// della password, una regola sudoers (installata una volta sola con i privilegi di
// amministratore) consente all'utente corrente di eseguire esattamente quel comando.
//
// Le versioni precedenti venivano avviate come root e lasciavano la cartella dei dati
// (database dei profili) di proprietà di root: la stessa procedura la restituisce all'utente.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

const SUDO: &str = "/usr/bin/sudo";
const TEE: &str = "/usr/bin/tee";
pub const HOSTS_PATH: &str = "/etc/hosts";
pub const SUDOERS_FILE: &str = "/etc/sudoers.d/hosts-manager";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostsPermission {
    // Il file hosts è scrivibile direttamente (es: app avviata come root)
    Writable,
    // La regola sudoers è attiva: scrittura tramite sudo senza password
    Sudo,
    // Nessun permesso di scrittura: va configurata la regola sudoers
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionStatus {
    pub hosts: HostsPermission,
    // false se la cartella dei dati (o il database) non è scrivibile dall'utente
    pub data_dir_writable: bool,
}

impl PermissionStatus {
    pub fn needs_setup(&self) -> bool {
        self.hosts == HostsPermission::Missing || !self.data_dir_writable
    }
}

pub fn check_permissions(data_dir: &Path) -> PermissionStatus {
    PermissionStatus {
        hosts: check_hosts_permission(),
        data_dir_writable: is_data_dir_writable(data_dir),
    }
}

fn check_hosts_permission() -> HostsPermission {
    if OpenOptions::new().write(true).open(HOSTS_PATH).is_ok() {
        HostsPermission::Writable
    } else if sudo_rule_active() {
        HostsPermission::Sudo
    } else {
        HostsPermission::Missing
    }
}

// La regola è attiva se il file sudoers esiste e sudo consente il comando.
// `sudo -l` da solo non basta: non chiede la password anche per comandi che la richiederebbero.
fn sudo_rule_active() -> bool {
    cfg!(unix)
        && Path::new(SUDOERS_FILE).exists()
        && Command::new(SUDO)
            .args(["-n", "-k", "-l", TEE, HOSTS_PATH])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}

// La cartella deve permettere di creare file (SQLite crea il journal accanto al database)
// e i file già presenti devono essere scrivibili. Una cartella non ancora creata va bene.
fn is_data_dir_writable(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return !dir.exists();
    };

    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    let dir_writable = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);

    dir_writable
        && entries
            .flatten()
            .filter(|e| e.path().is_file())
            .all(|e| OpenOptions::new().append(true).open(e.path()).is_ok())
}

// Scrive il contenuto nel file hosts tramite `sudo -n tee` (non chiede mai la password)
pub fn write_hosts_with_sudo(path: &Path, content: &[u8]) -> io::Result<()> {
    // La regola sudoers consente solo il percorso esatto
    if path != Path::new(HOSTS_PATH) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Percorso non consentito per sudo"));
    }

    let mut child = Command::new(SUDO)
        .args(["-n", TEE, HOSTS_PATH])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("stdin di sudo non disponibile"))?
        .write_all(content)?;

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Permesso negato su {}: configura i permessi da 'Gestisci Profili' ({}).",
                HOSTS_PATH,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}

// Nome dell'utente corrente, validato perché finisce nella regola sudoers e in un comando shell
fn current_user() -> io::Result<String> {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| io::Error::other("Impossibile determinare l'utente corrente"))?;

    let valid = !user.is_empty()
        && user != "root"
        && user.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(user)
    } else {
        Err(io::Error::other(format!("Nome utente non supportato: '{}'", user)))
    }
}

pub fn sudoers_rule(user: &str) -> String {
    format!(
        "# Creato da Hosts Manager: consente a {user} di riscrivere {HOSTS_PATH} senza password\n\
         {user} ALL=(root) NOPASSWD: {TEE} {HOSTS_PATH}\n"
    )
}

// Script eseguito come root: valida la regola con visudo prima di installarla
fn install_script(user: &str) -> String {
    let rule_lines: Vec<String> = sudoers_rule(user)
        .lines()
        .map(|l| format!("'{}'", l))
        .collect();

    format!(
        "tmp=$(/usr/bin/mktemp /tmp/hosts-manager-sudoers.XXXXXX) && \
         printf '%s\\n' {lines} > \"$tmp\" && \
         /usr/sbin/visudo -cf \"$tmp\" >/dev/null && \
         /usr/sbin/chown root:wheel \"$tmp\" && /bin/chmod 0440 \"$tmp\" && \
         /bin/mkdir -p /etc/sudoers.d && /bin/mv -f \"$tmp\" {SUDOERS_FILE} \
         || {{ /bin/rm -f \"$tmp\"; exit 1; }}",
        lines = rule_lines.join(" ")
    )
}

// Restituisce all'utente la cartella dei dati creata da una versione avviata come root
fn chown_script(user: &str, data_dir: &Path) -> io::Result<String> {
    let dir = data_dir.to_str().filter(|d| !d.contains('\'')).ok_or_else(|| {
        io::Error::other(format!("Percorso dei dati non supportato: {}", data_dir.display()))
    })?;
    Ok(format!("/usr/sbin/chown -R {user} '{dir}'"))
}

// Configura i permessi mancanti chiedendo una sola volta la password di amministratore
// (finestra di sistema macOS): regola sudoers per il file hosts e proprietà della cartella dei dati
pub fn setup_permissions(data_dir: &Path) -> Result<(), String> {
    let status = check_permissions(data_dir);
    let user = current_user().map_err(|e| e.to_string())?;

    let mut steps = Vec::new();
    if status.hosts == HostsPermission::Missing {
        steps.push(install_script(&user));
    }
    if !status.data_dir_writable {
        steps.push(chown_script(&user, data_dir).map_err(|e| e.to_string())?);
    }
    if steps.is_empty() {
        return Ok(());
    }

    let script = steps.iter().map(|s| format!("( {} )", s)).collect::<Vec<_>>().join(" && ");
    run_as_admin(&script)?;

    let status = check_permissions(data_dir);
    if status.hosts == HostsPermission::Missing {
        Err("Regola installata ma non attiva: verifica che /etc/sudoers includa /etc/sudoers.d.".to_string())
    } else if !status.data_dir_writable {
        Err(format!("La cartella {} non è ancora scrivibile.", data_dir.display()))
    } else {
        Ok(())
    }
}

pub fn remove_sudoers_rule() -> Result<(), String> {
    run_as_admin(&format!("/bin/rm -f {SUDOERS_FILE}"))
}

#[cfg(target_os = "macos")]
fn run_as_admin(script: &str) -> Result<(), String> {
    // Escape per una stringa AppleScript
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!("do shell script \"{}\" with administrator privileges", escaped))
        .output()
        .map_err(|e| format!("Impossibile avviare osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") {
            Err("Operazione annullata.".to_string())
        } else {
            Err(format!("Operazione non riuscita: {}", stderr.trim()))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn run_as_admin(_script: &str) -> Result<(), String> {
    let user = current_user().map_err(|e| e.to_string())?;
    Err(format!(
        "Configurazione automatica disponibile solo su macOS. Aggiungi con `sudo visudo -f {}`:\n{}",
        SUDOERS_FILE,
        sudoers_rule(&user)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_allows_only_tee_on_hosts() {
        assert_eq!(
            sudoers_rule("romeo").lines().last().unwrap(),
            "romeo ALL=(root) NOPASSWD: /usr/bin/tee /etc/hosts"
        );
    }

    #[test]
    fn install_script_validates_before_installing() {
        let script = install_script("romeo");
        let visudo = script.find("visudo -cf").unwrap();
        let mv = script.find("/bin/mv").unwrap();
        assert!(visudo < mv);
        assert!(script.contains("'romeo ALL=(root) NOPASSWD: /usr/bin/tee /etc/hosts'"));
    }

    #[test]
    fn rule_is_valid_sudoers_syntax() {
        // visudo -c funziona anche senza root su un file dell'utente
        let path = std::env::temp_dir().join(format!("hosts-manager-sudoers-{}", std::process::id()));
        std::fs::write(&path, sudoers_rule("romeo")).unwrap();
        let status = Command::new("/usr/sbin/visudo").arg("-cf").arg(&path).output();
        std::fs::remove_file(&path).unwrap();
        if let Ok(output) = status {
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    fn chown_script_quotes_path() {
        let script = chown_script("romeo", Path::new("/Users/romeo/Library/Application Support/hosts_manager")).unwrap();
        assert_eq!(script, "/usr/sbin/chown -R romeo '/Users/romeo/Library/Application Support/hosts_manager'");
        assert!(chown_script("romeo", Path::new("/tmp/it's")).is_err());
    }

    #[test]
    fn data_dir_writable_checks() {
        let dir = std::env::temp_dir().join(format!("hosts-manager-data-{}", std::process::id()));
        assert!(is_data_dir_writable(&dir)); // non ancora creata
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("profiles.db"), b"").unwrap();
        assert!(is_data_dir_writable(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_other_paths() {
        let err = write_hosts_with_sudo(Path::new("/etc/passwd"), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
}
