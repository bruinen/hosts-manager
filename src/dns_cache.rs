// Svuotamento della cache DNS di sistema, così le modifiche al file hosts hanno effetto subito.

use std::process::{Command, Stdio};

#[cfg(unix)]
use crate::privileges::{self, FLUSH_DNS_COMMAND};

// Esegue un comando senza output; Err(NotFound) se il programma non esiste
fn run(program: &str, args: &[&str]) -> std::io::Result<bool> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

    #[cfg(windows)]
    {
        // Evita che si apra una finestra della console
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.status().map(|s| s.success())
}

// macOS: svuota la cache di Directory Services e fa ricaricare mDNSResponder.
// Il secondo comando richiede root: va diretto se l'app è root, altrimenti tramite la regola sudoers.
#[cfg(target_os = "macos")]
pub fn flush_dns_cache() -> Result<(), String> {
    let _ = run("/usr/bin/dscacheutil", &["-flushcache"]);

    let (program, args) = (FLUSH_DNS_COMMAND[0], &FLUSH_DNS_COMMAND[1..]);
    if run(program, args).unwrap_or(false) || privileges::run_with_sudo(FLUSH_DNS_COMMAND, false) {
        Ok(())
    } else {
        Err("DNS cache not flushed: set up permissions in the sidebar.".to_string())
    }
}

// Windows: l'app gira come amministratore, quindi basta ipconfig
#[cfg(windows)]
pub fn flush_dns_cache() -> Result<(), String> {
    match run("ipconfig", &["/flushdns"]) {
        Ok(true) => Ok(()),
        Ok(false) => Err("DNS cache not flushed: run the app as administrator.".to_string()),
        Err(e) => Err(format!("DNS cache not flushed: {}", e)),
    }
}

// Linux e altri Unix: solo systemd-resolved mantiene una cache; senza resolvectl non c'è nulla da svuotare
#[cfg(all(unix, not(target_os = "macos")))]
pub fn flush_dns_cache() -> Result<(), String> {
    let (program, args) = (FLUSH_DNS_COMMAND[0], &FLUSH_DNS_COMMAND[1..]);
    match run(program, args) {
        Ok(true) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ if privileges::run_with_sudo(FLUSH_DNS_COMMAND, false) => Ok(()),
        _ => Err("DNS cache not flushed: add the sudoers rule to allow it.".to_string()),
    }
}
