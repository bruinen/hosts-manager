use iced::widget::{column,Text, text, button, text_input, row, scrollable};
use iced::{Theme,Task,color,Element, Length, Renderer}; // Importazioni aggiornate
use std::fs;
use std::fs::File;
use std::io::Write;
use std::fs::OpenOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    ip: String,
    hostname: String,
    comment: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message { // Deve essere pub
    ButtonPressed,
    InputChanged(String),
    WriteToHosts(String, Option<String>),
    DeleteEntry(usize),
    EditEntry(usize),
    EditIpChanged(String),
    EditHostnameChanged(String),
    SaveEditedEntry,
    CancelEdit,
    NoOp,
    SaveSuccess,
    SaveError(String),
}

#[derive(Debug, Default, Clone)] // Aggiungi Default
pub struct MyApp { // Deve essere pub
    pub input_text: String,
    pub file_lines: Vec<Line>,
    pub editing_index: Option<usize>,
    pub editing_ip: String,
    pub editing_hostname: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Comment(String),
    Entry(HostEntry),
    Empty,
}

fn load_hosts_entries() -> Vec<Line> {
    let hosts_path = "/etc/hosts";
    let contents = fs::read_to_string(hosts_path).unwrap_or_else(|e| {
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

fn write_hosts_entries_to_file(lines: &[Line]) -> Result<(), String> {
    let hosts_path = "/etc/hosts";

    let mut file = match File::create(hosts_path) {
        Ok(f) => f,
        Err(e) => return Err(format!("Impossibile aprire il file per la scrittura: {}", e)),
    };

    for line in lines {
        let line_to_write = match line {
            Line::Comment(s) => s.clone(),
            Line::Entry(entry) => {
                let mut formatted_line = format!("{} {}", entry.ip, entry.hostname);
                if let Some(comment) = &entry.comment {
                    formatted_line.push_str(&format!(" #{}", comment));
                }
                formatted_line
            },
            Line::Empty => String::new(),
        };

        if let Err(e) = writeln!(file, "{}", line_to_write) {
            return Err(format!("Errore nella scrittura del record: {}", e));
        }
    }

    Ok(())
}

fn update(state: &mut MyApp, message: Message) -> Task<Message> { // Nuova firma!
    state.error_message = None;
    match message {
        Message::ButtonPressed => {
            let hostname = state.input_text.clone();

            return Task::perform(async move {
                use dns_lookup::lookup_host;
                let ips = lookup_host(&hostname);

                match ips {
                    Ok(ips) => Ok((hostname, ips)),
                    Err(e) => Err(format!("Errore nel lookup: {}", e)),
                }
            }, move |result| {
                match result {
                    Ok((hostname, ips)) => {
                        println!("Trovati IP per {}: {:?}", hostname, ips);
                        Message::WriteToHosts(hostname, ips.first().map(|ip| ip.to_string()))
                    }
                    Err(e) => {
                        Message::SaveError(e)
                    }
                }
            });
        }
        Message::InputChanged(new_text) => {
            state.input_text = new_text;
        }
        Message::WriteToHosts(hostname, ip_option) => {
            if let Some(ip) = ip_option {
                let new_entry = format!("\n{} {}", ip, hostname);
                let hosts_path = "/etc/hosts";

                let mut file = match OpenOptions::new().append(true).open(hosts_path) {
                    Ok(f) => f,
                    Err(e) => {
                        println!("Errore nell'apertura del file hosts: {}", e);
                        return Task::none();
                    }
                };

                match file.write_all(new_entry.as_bytes()) {
                    Ok(_) => {
                        println!("Riga aggiunta con successo al file hosts: {}", new_entry);
                        state.file_lines.push(Line::Entry(HostEntry {
                            ip: ip.clone(),
                            hostname: hostname.clone(),
                            comment: None,
                        }));
                    }
                    Err(e) =>state.error_message = Some(format!("Errore nella scrittura del file hosts: {}", e)),
                }

            } else {
                state.error_message = Some(String::from("Impossibile trovare un IP per l'hostname fornito."));
            }
        }
        Message::DeleteEntry(index) => {
            if index < state.file_lines.len() {
                state.file_lines.remove(index);
                println!("Record eliminato dallo stato.");

                let entries_to_save = state.file_lines.clone();

                return Task::perform(async move {
                    write_hosts_entries_to_file(&entries_to_save)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                }, |result| {
                    match result {
                        Ok(_) => Message::SaveSuccess,
                        Err(e) => Message::SaveError(e),
                    }
                });
            }
        }
        Message::EditEntry(index) => {
            if let Some(Line::Entry(entry)) = state.file_lines.get(index) {
                state.editing_index = Some(index);
                state.editing_ip = entry.ip.clone();
                state.editing_hostname = entry.hostname.clone();
                println!("Modalità di modifica attivata per il record: {} {}", entry.ip, entry.hostname);
            }
        }
        Message::EditIpChanged(new_ip) => {
            state.editing_ip = new_ip;
        }
        Message::EditHostnameChanged(new_hostname) => {
            state.editing_hostname = new_hostname;
        }
        Message::SaveEditedEntry => {
            if let Some(index) = state.editing_index {
                if let Some(Line::Entry(entry)) = state.file_lines.get_mut(index) {
                    entry.ip = state.editing_ip.clone();
                    entry.hostname = state.editing_hostname.clone();

                    state.editing_index = None;
                    state.editing_ip = String::new();
                    state.editing_hostname = String::new();

                    let entries_to_save = state.file_lines.clone();
                    return Task::perform(async move {
                        write_hosts_entries_to_file(&entries_to_save)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    }, |result| {
                        match result {
                            Ok(_) => Message::SaveSuccess,
                            Err(e) => Message::SaveError(e),
                        }
                    });
                }
            }
        }
        Message::CancelEdit => {
            state.editing_index = None;
            state.editing_ip = String::new();
            state.editing_hostname = String::new();
            println!("Modifica annullata.");
        }
        Message::NoOp => {}
        Message::SaveSuccess => {
            state.error_message = None; // Rimuovi l'errore in caso di successo
            println!("File hosts salvato con successo!");
        }
        Message::SaveError(e) => {
            state.error_message = Some(e);
        }
    }
    Task::none()
}

fn view(state: &MyApp) -> Element<Message> { // Nuova firma!
    let error_label: Text<'_, Theme, Renderer> = match &state.error_message {
        Some(msg) => text(msg)
            .size(16)
            .color(color!(0xff0000)), // Testo rosso
        None => text(""), // Se non c'è errore, l'etichetta è vuota
    };

    let entries_list: Vec<Element<Message>> = state.file_lines.iter().enumerate().filter_map(|(index, line)| {
        match line {
            Line::Entry(entry) => {
                if state.editing_index == Some(index) {
                    Some(row![
                        text_input("IP", &state.editing_ip)
                            .on_input(Message::EditIpChanged)
                            .width(Length::Fill),
                        text_input("Hostname", &state.editing_hostname)
                            .on_input(Message::EditHostnameChanged)
                            .width(Length::Fill),
                        button("Salva").on_press(Message::SaveEditedEntry),
                        button("Annulla").on_press(Message::CancelEdit),
                    ]
                        .spacing(10)
                        .align_y(iced::Alignment::Center)
                        .padding(5)
                        .into())
                } else {
                    Some(row![
                        text(format!("{} {}", entry.ip, entry.hostname)),
                        button("Modifica").on_press(Message::EditEntry(index)),
                        button("Elimina").on_press(Message::DeleteEntry(index)),
                    ]
                        .spacing(10)
                        .align_y(iced::Alignment::Center)
                        .padding(5)
                        .into())
                }
            },
            _ => None,
        }
    }).collect();

    let scrollable_entries = scrollable(column(entries_list).spacing(5));

    let content = column![
        text("Inserisci un hostname da scrivere nel file hosts:"),
        row![
            text_input(
                "Es: example.com",
                &state.input_text,
            )
            .on_input(Message::InputChanged)
            .width(Length::Fill),
            button("Inserisci").on_press(Message::ButtonPressed),
        ]
        .spacing(10),
        error_label,
        text("Record nel file hosts:").size(20),
        scrollable_entries,
    ]
        .spacing(10)
        .padding(10);

    content.into()
}

fn theme(_state: &MyApp) -> Theme {
    Theme::TokyoNight
}

pub fn main() -> iced::Result {
    // Carichiamo lo stato iniziale
    let initial_state = MyApp {
        input_text: String::new(),
        file_lines: load_hosts_entries(),
        editing_index: None,
        editing_ip: String::new(),
        editing_hostname: String::new(),
        error_message: None,
    };

    // Avviamo l'applicazione
    iced::application("Hosts manager", update,view)        
        .theme(theme)
        .run_with(|| { // Usiamo una closure per creare e restituire lo stato
            (initial_state,Task::none()) // Restituiamo lo stato e un Task
        })
}