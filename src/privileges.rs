// Gestione dei permessi per scrivere il file hosts senza avviare l'app come root.
//
// L'app gira come utente normale. Se la scrittura diretta di /etc/hosts non è permessa,
// il contenuto viene passato a `sudo -n /usr/bin/tee /etc/hosts`. Per evitare la richiesta
// della password, una regola sudoers (installata una volta sola con i privilegi di
// amministratore) consente all'utente corrente di eseguire esattamente quel comando.

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

pub fn check_hosts_permission() -> HostsPermission {
    if OpenOptions::new().write(true).open(HOSTS_PATH).is_ok() {
        HostsPermission::Writable
    } else if sudo_rule_active() {
        HostsPermission::Sudo
    } else {
        HostsPermission::Missing
    }
}

// `sudo -n -k -l <comando>` termina con successo solo se il comando è consentito senza password
// (-k ignora le credenziali in cache, che darebbero un falso positivo per qualche minuto)
fn sudo_rule_active() -> bool {
    cfg!(unix)
        && Command::new(SUDO)
            .args(["-n", "-k", "-l", TEE, HOSTS_PATH])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
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

// Installa la regola sudoers chiedendo la password di amministratore (finestra di sistema macOS)
pub fn install_sudoers_rule() -> Result<(), String> {
    let user = current_user().map_err(|e| e.to_string())?;
    run_as_admin(&install_script(&user))?;

    if sudo_rule_active() {
        Ok(())
    } else {
        Err("Regola installata ma non attiva: verifica che /etc/sudoers includa /etc/sudoers.d.".to_string())
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
    fn refuses_other_paths() {
        let err = write_hosts_with_sudo(Path::new("/etc/passwd"), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
}
