use iced::{
    executor, Element, Theme, Settings, Length,
};
use iced::widget::{column, text, button, text_input, row, scrollable};
use iced::Color;

use std::fs;
use std::fs::File;
use std::io::Write;
use std::fs::OpenOptions;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostEntry {
    ip: String,
    hostname: String,
    comment: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
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

struct MyApp {
    input_text: String,
    file_lines: Vec<Line>,
    editing_index: Option<usize>,
    editing_ip: String,
    editing_hostname: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Comment(String),
    Entry(HostEntry),
    Empty,
}

fn load_hosts_entries() -> Vec<Line> {
    let hosts_path = "/etc/hosts";
    let contents = match fs::read_to_string(hosts_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Errore nella lettura del file hosts: {}", e);
            return Vec::new();
        }
    };

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

pub fn main() -> iced::Result {
    iced::application("Hosts manager", update, view)
    .theme(theme)
    .run()
}



fn update(value: &mut u64, message: Message) {
    match message {
        Message::ButtonPressed => {
            // Otteniamo l'hostname dallo stato
            let hostname = self.input_text.clone();

            // Usiamo il comando per eseguire l'operazione in un task separato
            // in modo da non bloccare l'interfaccia utente.
            return Command::perform(async move {
                // Eseguiamo la ricerca DNS in modo asincrono
                use dns_lookup::lookup_host;
                let ips = lookup_host(&hostname);

                // Restituiamo il risultato
                match ips {
                    Ok(ips) => Ok((hostname, ips)),
                    Err(e) => Err(format!("Errore nel lookup: {}", e)),
                }
            }, move |result| {
                // Una volta completato il task, inviamo un nuovo messaggio
                match result {
                    Ok((hostname, ips)) => {
                        println!("Trovati IP per {}: {:?}", hostname, ips);
                        Message::WriteToHosts(hostname, ips.first().map(|ip| ip.to_string()))
                    }
                    Err(e) => {
                        println!("{}", e);
                        Message::NoOp // Messaggio nullo
                    }
                }
            });
        }
        Message::InputChanged(new_text) => {
            self.input_text = new_text;
        }
        Message::WriteToHosts(hostname, ip_option) => {
            // Qui scriviamo nel file hosts!
            if let Some(ip) = ip_option {
                let new_entry = format!("\n{} {}", ip, hostname);
                let hosts_path = "/etc/hosts";

                // Proviamo ad aprire il file in modalità append
                let mut file = match OpenOptions::new().append(true).open(hosts_path) {
                    Ok(f) => f,
                    Err(e) => {
                        println!("Errore nell'apertura del file hosts: {}", e);
                        return Command::none();
                    }
                };

                // Proviamo a scrivere la nuova riga
                match file.write_all(new_entry.as_bytes()) {
                    Ok(_) => {
                        println!("Riga aggiunta con successo al file hosts: {}", new_entry);
                        // Dopo aver scritto con successo, aggiorniamo la lista di record
                        self.file_lines.push(Line::Entry(HostEntry {
                            ip: ip.clone(),
                            hostname: hostname.clone(),
                            comment: None,
                        }));
                    }
                    Err(e) => println!("Errore nella scrittura del file hosts: {}", e),
                }
            } else {
                println!("Impossibile trovare un IP per l'hostname fornito.");
            }
        }
        Message::DeleteEntry(index) => {
            // Rimuoviamo l'elemento dalla lista usando l'indice
            if index < self.file_lines.len() {
                self.file_lines.remove(index);
                println!("Record eliminato dallo stato.");

                // Clona la lista dei record per passarla al task asincrono
                let entries_to_save = self.file_lines.clone();

                // Ora, dobbiamo salvare il nuovo stato nel file hosts
                // Eseguiamo il salvataggio in un thread separato
                return Command::perform(async move {
                    // Chiamiamo la funzione per salvare la lista nel file
                    // Passiamo la copia dei dati
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
            if let Some(Line::Entry(entry)) = self.file_lines.get(index) {
                self.editing_index = Some(index);
                self.editing_ip = entry.ip.clone();
                self.editing_hostname = entry.hostname.clone();
                println!("Modalità di modifica attivata per il record: {} {}", entry.ip, entry.hostname);
            }
        }
        Message::EditIpChanged(new_ip) => {
            self.editing_ip = new_ip;
        }
        Message::EditHostnameChanged(new_hostname) => {
            self.editing_hostname = new_hostname;
        }
        Message::SaveEditedEntry => {
            if let Some(index) = self.editing_index {
                if let Some(Line::Entry(entry)) = self.file_lines.get_mut(index) {
                    entry.ip = self.editing_ip.clone();
                    entry.hostname = self.editing_hostname.clone();

                    // Resettiamo lo stato di modifica
                    self.editing_index = None;
                    self.editing_ip = String::new();
                    self.editing_hostname = String::new();

                    // Salviamo il file aggiornato
                    let entries_to_save = self.file_lines.clone();
                    return Command::perform(async move {
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
            // Resettiamo lo stato di modifica senza salvare
            self.editing_index = None;
            self.editing_ip = String::new();
            self.editing_hostname = String::new();
            println!("Modifica annullata.");
        }
        Message::NoOp => {}
        Message::SaveSuccess => {
            println!("File hosts salvato con successo!");
        }
        Message::SaveError(e) => {
            println!("Errore nel salvataggio del file hosts: {}", e);
        }
    }
}

fn view(value: &u64) -> Column<Message> {
    let entries_list: Vec<Element<Message>> = self.file_lines.iter().enumerate().filter_map(|(index, line)| {
        match line {
            Line::  Entry(entry) => {
                if self.editing_index == Some(index) {
                    // Se siamo in modalità modifica, mostriamo le caselle di testo
                    Some(row![
                        text_input("IP", &self.editing_ip)
                            .on_input(Message::EditIpChanged)
                            .width(Length::Fill),
                        text_input("Hostname", &self.editing_hostname)
                            .on_input(Message::EditHostnameChanged)
                            .width(Length::Fill),
                        button("Salva").on_press(Message::SaveEditedEntry),
                        button("Annulla").on_press(Message::CancelEdit),
                    ]
                    .spacing(10)
                    ..align_y(iced::Alignment::Center)
                    .padding(5)
                    .into())
                } else {
                    // Se non siamo in modalità modifica, mostriamo il testo normale
                    Some(row![
                        text(format!("{} {}", entry.ip, entry.hostname)),
                        button("Modifica").on_press(Message::EditEntry(index)),
                        button("Elimina").on_press(Message::DeleteEntry(index)),
                    ]
                    .spacing(10)
                    .align_items(iced::Alignment::Center)
                    .padding(5)
                    .into())
                }
            },
            _ => None,
        }
    }).collect();

    let scrollable_entries = scrollable(column(entries_list).spacing(5));

    let content = column![
        text("Inserisci un hostname da bloccare nel file hosts:"),
        row![
            text_input(
                "Es: example.com",
                &self.input_text,
            )
            .on_input(Message::InputChanged)
            .width(Length::Fill),
            button("Blocca").on_press(Message::ButtonPressed),
        ]
        .spacing(10),
        text("Record nel file hosts:").size(20).style(Color::from_rgb(0.5, 0.5, 0.5)),
        scrollable_entries,
    ]
    .spacing(10)
    .padding(10);

    content.into()
}


fn theme(state: &State) -> Theme {
    Theme::TokyoNight
}