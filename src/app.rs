use iced::{Alignment, Color, Element, Length, Task, Theme,
           widget::{column, Text, text, button, text_input, row, scrollable, container, Space}, Settings, Renderer};
use crate::host_manager::{Line, write_hosts_entries_to_file, Entry, hostname_matches};
use crate::{host_manager, profile_view, db_manager};
use crate::db_manager::{update_profile, Profile};
use crate::dns_lookup::resolve_hostname_with_specific_dns;
use crate::privileges::{self, PermissionStatus};

// Server DNS proposto all'avvio, sovrascrivibile con la variabile d'ambiente HOSTS_MANAGER_DNS
const DEFAULT_DNS_SERVER: &str = "10.10.10.10";

// Enum for the current view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Main,
    Profiles,
}
#[derive(Debug, Clone)]
pub enum Message {
    InputIpChanged(String),
    InputHostnameChanged(String),
    ManualAddButtonPressed,

    InputChanged(String),
    InputDNSChanged(String),
    DnsLookupButtonPressed,
    DnsLookupResult(Result<Entry, String>),

    DeleteEntry(usize),
    ToggleEntry(usize),
    EditEntry(usize),
    EditIpChanged(String),
    EditHostnameChanged(String),
    SaveEditedEntry,
    CancelEdit,
    SaveSuccess,
    SaveError(String),
    ProfileSelected(String),
    NewProfileNameChanged(String),
    CreateProfileButtonPressed,
    ProfileCreated(Result<Profile, String>),
    LoadProfiles,
    LoadProfilesResult(Result<Vec<Profile>, String>),
    ShowMainView,
    ShowProfilesView,
    DeleteProfile(String),
    ProfileDeleted(Result<String, String>),
    UpdateDatabaseResult(Result<(), String>),

    ExportProfilesButtonPressed,
    ImportProfilesButtonPressed,
    ExportProfilesResult(Result<(), String>),
    ImportProfilesResult(Result<Profile, String>),

    SearchQueryChanged(String),
    ClearSearch,

    CheckHostsPermission,
    HostsPermissionChecked(PermissionStatus),
    InstallSudoersPressed,
    RemoveSudoersPressed,
    SudoersResult(Result<String, String>),
}

impl Message {
    // Risultati di operazioni asincrone: non devono cancellare il messaggio di stato corrente
    fn is_async_result(&self) -> bool {
        matches!(self,
            Message::DnsLookupResult(_)
            | Message::SaveSuccess
            | Message::SaveError(_)
            | Message::ProfileCreated(_)
            | Message::LoadProfiles
            | Message::LoadProfilesResult(_)
            | Message::ProfileDeleted(_)
            | Message::UpdateDatabaseResult(_)
            | Message::ExportProfilesResult(_)
            | Message::ImportProfilesResult(_)
            | Message::CheckHostsPermission
            | Message::HostsPermissionChecked(_)
            | Message::SudoersResult(_))
    }
}

#[derive(Debug, Default)]
pub struct MyApp {
    pub input_text: String,
    pub input_text_dns: String,
    pub input_ip: String,
    pub input_hostname: String,
    pub editing_index: Option<usize>,
    pub editing_ip: String,
    pub editing_hostname: String,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub profiles: Vec<Profile>,
    pub selected_profile_id: Option<String>,
    pub new_profile_name: String,
    pub view: View,
    pub search_query: String,
    pub hosts_permission: Option<PermissionStatus>,
}

impl MyApp {
    pub fn selected_profile(&self) -> Option<&Profile> {
        let id = self.selected_profile_id.as_ref()?;
        self.profiles.iter().find(|p| &p.id == id)
    }

    fn selected_profile_mut(&mut self) -> Option<&mut Profile> {
        let id = self.selected_profile_id.as_ref()?;
        self.profiles.iter_mut().find(|p| &p.id == id)
    }

    fn set_success(&mut self, msg: impl Into<String>) {
        self.error_message = None;
        self.success_message = Some(msg.into());
    }

    fn set_error(&mut self, msg: impl Into<String>) {
        self.success_message = None;
        self.error_message = Some(msg.into());
    }

    fn cancel_edit(&mut self) {
        self.editing_index = None;
        self.editing_ip.clear();
        self.editing_hostname.clear();
    }
}


// Scrive le righe nel file hosts e notifica l'esito
fn write_hosts_task(entries: Vec<Line>) -> Task<Message> {
    Task::perform(async move {
        write_hosts_entries_to_file(&entries)
            .map_err(|e| e.to_string())
    }, |result| {
        match result {
            Ok(_) => Message::SaveSuccess,
            Err(e) => Message::SaveError(e),
        }
    })
}

// Esegue un'operazione sul database in background, riportando l'errore come stringa
fn db_task<T: Send + 'static>(
    operation: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    on_result: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(async move {
        let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
        operation(&conn).map_err(|e| e.to_string())
    }, on_result)
}

// Esegue un'operazione bloccante (processi esterni, dialoghi di sistema) fuori dai thread async
fn blocking_task<T: Send + 'static>(
    operation: impl FnOnce() -> T + Send + 'static,
    on_result: impl Fn(T) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(async move {
        tokio::task::spawn_blocking(operation).await.expect("task bloccante interrotto")
    }, on_result)
}

// Applica una modifica ai record del profilo selezionato e la salva su file hosts e database
fn update_entries(state: &mut MyApp, change: impl FnOnce(&mut Vec<Line>)) -> Task<Message> {
    let Some(profile) = state.selected_profile_mut() else {
        state.set_error("Nessun profilo selezionato.");
        return Task::none();
    };

    change(&mut profile.hosts);
    let profile_to_update = profile.clone();

    Task::batch([
        write_hosts_task(profile_to_update.hosts.clone()),
        db_task(move |conn| update_profile(conn, &profile_to_update), Message::UpdateDatabaseResult),
    ])
}


fn update(state: &mut MyApp, message: Message) -> Task<Message> {
    if !message.is_async_result() {
        state.error_message = None;
        state.success_message = None;
    }

    match message {

        Message::InputChanged(value) => {
            state.input_text = value;
        }
        Message::InputDNSChanged(value) => {
            state.input_text_dns = value;
        }
        Message::InputIpChanged(value) => {
            state.input_ip = value;
        }
        Message::InputHostnameChanged(value) => {
            state.input_hostname = value;
        }
        Message::ManualAddButtonPressed => {
            match Entry::new(&state.input_ip, &state.input_hostname) {
                Ok(entry) => {
                    state.input_ip.clear();
                    state.input_hostname.clear();
                    return update_entries(state, |hosts| hosts.push(Line::Entry(entry)));
                }
                Err(e) => state.set_error(e),
            }
        }
        Message::DnsLookupButtonPressed => {
            let hostname = state.input_text.trim().to_string();
            let dns_server = state.input_text_dns.clone();
            if hostname.is_empty() {
                state.set_error("L'hostname è obbligatorio per il DNS lookup.");
                return Task::none();
            }
            state.set_success("Ricerca IP in corso...");

            return Task::perform(async move {
                let ip = resolve_hostname_with_specific_dns(&hostname, &dns_server).await?;
                Entry::new(&ip, &hostname)
            }, Message::DnsLookupResult);
        }
        Message::DnsLookupResult(Ok(entry)) => {
            state.set_success(format!("IP trovato: {} → {}", entry.hostname, entry.ip));
            if state.input_text.trim() == entry.hostname {
                state.input_text.clear();
            }
            return update_entries(state, |hosts| hosts.push(Line::Entry(entry)));
        }
        Message::DnsLookupResult(Err(e)) => {
            state.set_error(e);
        }

        Message::DeleteEntry(index) => {
            // Mantieni coerente l'indice del record in modifica
            state.editing_index = match state.editing_index {
                Some(i) if i == index => None,
                Some(i) if i > index => Some(i - 1),
                other => other,
            };
            return update_entries(state, |hosts| {
                if index < hosts.len() {
                    hosts.remove(index);
                }
            });
        }
        Message::ToggleEntry(index) => {
            return update_entries(state, |hosts| {
                if let Some(Line::Entry(entry)) = hosts.get_mut(index) {
                    entry.enabled = !entry.enabled;
                }
            });
        }
        Message::EditEntry(index) => {
            let entry = match state.selected_profile().and_then(|p| p.hosts.get(index)) {
                Some(Line::Entry(entry)) => entry.clone(),
                _ => return Task::none(),
            };
            state.editing_index = Some(index);
            state.editing_hostname = entry.names();
            state.editing_ip = entry.ip;
        }
        Message::EditIpChanged(new_ip) => {
            state.editing_ip = new_ip;
        }
        Message::EditHostnameChanged(new_hostname) => {
            state.editing_hostname = new_hostname;
        }
        Message::SaveEditedEntry => {
            let Some(index) = state.editing_index else {
                return Task::none();
            };
            match Entry::new(&state.editing_ip, &state.editing_hostname) {
                Ok(edited) => {
                    state.cancel_edit();
                    return update_entries(state, |hosts| {
                        // Mantiene stato (attivo/disattivo) e commento del record originale
                        if let Some(Line::Entry(entry)) = hosts.get_mut(index) {
                            entry.ip = edited.ip;
                            entry.hostname = edited.hostname;
                            entry.aliases = edited.aliases;
                        }
                    });
                }
                Err(e) => state.set_error(e),
            }
        }

        Message::CancelEdit => {
            state.cancel_edit();
        }
        Message::SaveSuccess => {
            // Non sovrascrivere un messaggio più specifico (es: IP trovato, profilo attivato)
            if state.error_message.is_none() && state.success_message.is_none() {
                state.set_success("Modifiche salvate.");
            }
        }
        Message::SaveError(e) => {
            state.set_error(e);
        }
        Message::LoadProfiles => {
            return db_task(db_manager::get_all_profiles, |result| {
                Message::LoadProfilesResult(result.map_err(|e| format!("Errore nel caricamento dei profili: {}", e)))
            });
        }
        Message::LoadProfilesResult(Ok(profiles)) => {
            if profiles.is_empty() {
                // Primo avvio: crea il profilo Default dal file hosts di sistema
                state.set_success(format!("Creazione del profilo '{}'...", db_manager::DEFAULT_PROFILE_NAME));
                return db_task(|conn| {
                    let hosts = host_manager::load_hosts_entries();
                    db_manager::create_profile(conn, db_manager::DEFAULT_PROFILE_NAME, &hosts)
                }, |result| match result {
                    Ok(_) => Message::LoadProfiles,
                    Err(e) => Message::SaveError(format!("Errore nella creazione del profilo di default: {}", e)),
                });
            }

            let selected = profiles.iter().find(|p| p.is_active).or(profiles.first());
            if let Some(profile) = selected {
                state.set_success(format!("Profilo caricato: {}", profile.name));
                state.selected_profile_id = Some(profile.id.clone());
            }
            state.profiles = profiles;
            state.cancel_edit();
        }
        Message::LoadProfilesResult(Err(e)) => {
            state.set_error(e);
        }

        Message::ProfileSelected(profile_id) => {
            let Some(profile) = state.profiles.iter().find(|p| p.id == profile_id) else {
                return Task::none();
            };
            let entries_to_save = profile.hosts.clone();
            state.set_success(format!("Profilo attivato: {}", profile.name));

            for p in &mut state.profiles {
                p.is_active = p.id == profile_id;
            }
            state.selected_profile_id = Some(profile_id.clone());
            state.cancel_edit();

            return Task::batch([
                write_hosts_task(entries_to_save),
                db_task(move |conn| db_manager::set_active_profile(conn, &profile_id), |result| {
                    Message::UpdateDatabaseResult(result.map_err(|e| format!("Errore nell'attivazione del profilo: {}", e)))
                }),
            ]);
        }
        Message::NewProfileNameChanged(name) => {
            state.new_profile_name = name;
        }
        Message::CreateProfileButtonPressed => {
            let name = state.new_profile_name.trim().to_string();
            if name.is_empty() {
                state.set_error("Il nome del profilo non può essere vuoto.");
                return Task::none();
            }
            if state.profiles.iter().any(|p| p.name == name) {
                state.set_error(format!("Un profilo con il nome '{}' esiste già.", name));
                return Task::none();
            }

            let hosts = vec![Line::Entry(Entry::localhost())];
            return db_task(move |conn| db_manager::create_profile(conn, &name, &hosts), |result| {
                Message::ProfileCreated(result.map_err(|e| format!("Errore nella creazione del profilo: {}", e)))
            });
        }
        Message::ProfileCreated(Ok(profile)) => {
            state.set_success(format!("Profilo creato: {}", profile.name));
            state.new_profile_name.clear();
            state.profiles.push(profile);
        }
        Message::ProfileCreated(Err(e)) => {
            state.set_error(e);
        }
        Message::ShowMainView => {
            state.view = View::Main;
        }
        Message::ShowProfilesView => {
            state.view = View::Profiles;
        }
        Message::DeleteProfile(profile_id) => {
            let Some(profile) = state.profiles.iter().find(|p| p.id == profile_id) else {
                return Task::none();
            };
            if profile.is_default() {
                state.set_error(format!("Impossibile eliminare il profilo '{}'.", db_manager::DEFAULT_PROFILE_NAME));
                return Task::none();
            }
            if state.selected_profile_id.as_ref() == Some(&profile_id) {
                state.set_error("Impossibile eliminare il profilo attivo. Seleziona un altro profilo prima di procedere.");
                return Task::none();
            }

            state.set_success("Eliminazione del profilo...");

            return db_task(move |conn| {
                db_manager::delete_profile(conn, &profile_id)?;
                Ok(profile_id)
            }, |result| {
                Message::ProfileDeleted(result.map_err(|e| format!("Errore nell'eliminazione del profilo: {}", e)))
            });
        }
        Message::ProfileDeleted(Ok(profile_id)) => {
            state.profiles.retain(|p| p.id != profile_id);
            state.set_success("Profilo eliminato.");
        }
        Message::ProfileDeleted(Err(e)) => {
            state.set_error(e);
        }
        Message::UpdateDatabaseResult(Ok(_)) => {
        }
        Message::UpdateDatabaseResult(Err(e)) => {
            state.set_error(format!("Errore di aggiornamento del database: {}", e));
        }

        Message::ExportProfilesButtonPressed => {
            let Some(profile_to_export) = state.selected_profile().cloned() else {
                state.set_error("Seleziona un profilo da esportare.");
                return Task::none();
            };
            state.set_success("Apertura finestra di dialogo...");

            return Task::perform(async move {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON Profile", &["json"])
                    .set_file_name(format!("{}.json", profile_to_export.name))
                    .save_file()
                    .await
                    .ok_or("Operazione di esportazione annullata.")?;

                let json_data = serde_json::to_string_pretty(&profile_to_export)
                    .map_err(|e| format!("Errore di serializzazione: {}", e))?;

                std::fs::write(file.path(), json_data)
                    .map_err(|e| format!("Errore di scrittura del file: {}", e))
            }, Message::ExportProfilesResult);
        }
        Message::ImportProfilesButtonPressed => {
            state.set_success("Apertura finestra di dialogo...");

            return Task::perform(async {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON Profile", &["json"])
                    .pick_file()
                    .await
                    .ok_or("Operazione di importazione annullata.")?;

                let json_data = std::fs::read_to_string(file.path())
                    .map_err(|e| format!("Errore di lettura del file: {}", e))?;

                let imported_profile: Profile = serde_json::from_str(&json_data)
                    .map_err(|e| format!("Errore di deserializzazione: {}", e))?;

                let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                db_manager::import_profile(&conn, &imported_profile)
                    .map_err(|e| format!("Errore di importazione: {}", e))
            }, Message::ImportProfilesResult);
        }
        Message::ExportProfilesResult(Ok(_)) => {
            state.set_success("Profilo esportato con successo!");
        }
        Message::ExportProfilesResult(Err(e)) => {
            state.set_error(e);
        }

        Message::ImportProfilesResult(Ok(profile)) => {
            state.set_success(format!("Profilo importato con successo: {}", profile.name));
            state.profiles.push(profile);
        }
        Message::ImportProfilesResult(Err(e)) => {
            state.set_error(e);
        }

        Message::SearchQueryChanged(query) => {
            state.search_query = query;
        }
        Message::ClearSearch => {
            state.search_query.clear();
        }

        Message::CheckHostsPermission => {
            return blocking_task(|| privileges::check_permissions(db_manager::data_dir()), Message::HostsPermissionChecked);
        }
        Message::HostsPermissionChecked(permission) => {
            state.hosts_permission = Some(permission);
        }
        Message::InstallSudoersPressed => {
            state.set_success("Inserisci la password di amministratore nella finestra di sistema...");
            return blocking_task(
                || privileges::setup_permissions(db_manager::data_dir()).map(|_| "Permessi configurati.".to_string()),
                Message::SudoersResult,
            );
        }
        Message::RemoveSudoersPressed => {
            state.set_success("Inserisci la password di amministratore nella finestra di sistema...");
            return blocking_task(
                || privileges::remove_sudoers_rule().map(|_| "Regola sudoers rimossa.".to_string()),
                Message::SudoersResult,
            );
        }
        Message::SudoersResult(result) => {
            match result {
                Ok(msg) => state.set_success(msg),
                Err(e) => state.set_error(e),
            }
            return Task::done(Message::CheckHostsPermission);
        }

    }

    Task::none()
}
pub fn view(state: &MyApp) -> Element<'_, Message> {
    match state.view {
        View::Main => main_view(state),
        View::Profiles => profile_view::view(state),
    }
}

// Messaggio di stato (rosso per gli errori, verde per il successo), condiviso tra le viste
pub fn status_label(state: &MyApp) -> Text<'_, Theme, Renderer> {
    if let Some(msg) = &state.error_message {
        text(msg).size(16).color(Color::from_rgb(0.8, 0.2, 0.2))
    } else if let Some(msg) = &state.success_message {
        text(msg).size(16).color(Color::from_rgb(0.2, 0.7, 0.2))
    } else {
        text("")
    }
}


fn main_view(state: &MyApp) -> Element<'_, Message> {
    let selected_profile = state.selected_profile();

    let selected_profile_name = selected_profile
        .map(|p| p.name.as_str())
        .unwrap_or("Nessun profilo selezionato");

    let profile_info_row = row![
        text(format!("Profilo attuale: {}", selected_profile_name))
            .size(18)
            .color(Color::from_rgb(0.9, 0.9, 0.9)),
        Space::with_width(Length::Fill),
        button("Gestisci Profili")
            .on_press(Message::ShowProfilesView)
            .width(Length::Shrink),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let dns_lookup_section = row![
        text_input(
            "Hostname per DNS Lookup (es: google.com)",
            &state.input_text,
        )
        .on_input(Message::InputChanged)
        .on_submit(Message::DnsLookupButtonPressed)
        .width(Length::Fill),
         text_input(
            "DNS server (vuoto = DNS di sistema)",
            &state.input_text_dns,
        )
        .on_input(Message::InputDNSChanged)
        .width(Length::Fill),
        button("Cerca IP (DNS)").on_press(Message::DnsLookupButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let manual_add_section = row![
        text_input(
            "IP (es: 192.168.1.1)",
            &state.input_ip,
        )
        .on_input(Message::InputIpChanged)
        .width(Length::Fill),
        text_input(
            "Hostname, anche più di uno (es: server.local api.local)",
            &state.input_hostname,
        )
        .on_input(Message::InputHostnameChanged)
        .on_submit(Message::ManualAddButtonPressed)
        .width(Length::Fill),
        button("Aggiungi Manuale").on_press(Message::ManualAddButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let add_host_section = container(
        column![
            text("Aggiungi un nuovo record:").size(22).color(Color::from_rgb(0.1, 0.5, 0.8)),
            dns_lookup_section,
            text("oppure").size(14).color(Color::from_rgb(0.5, 0.5, 0.5)),
            manual_add_section,
            status_label(state),
        ]
            .spacing(10)
    )
        .padding(15)
        .style(container::rounded_box);

    let is_searching = !state.search_query.trim().is_empty();
    let hosts: &[Line] = selected_profile.map(|p| p.hosts.as_slice()).unwrap_or(&[]);

    // Durante la ricerca mostra solo i record il cui hostname corrisponde,
    // nascondendo commenti e righe vuote. L'indice resta quello originale
    // dei record del profilo, così Modifica/Elimina agiscono sul record corretto.
    let entries: Vec<Element<Message>> = hosts
        .iter()
        .enumerate()
        .filter(|(_, line)| match line {
            Line::Entry(entry) => hostname_matches(entry, &state.search_query),
            _ => !is_searching,
        })
        .map(|(index, line)| {
            match line {
                Line::Entry(entry) => {
                    if state.editing_index == Some(index) {
                        row![
                            text_input("IP", &state.editing_ip)
                                .on_input(Message::EditIpChanged)
                                .on_submit(Message::SaveEditedEntry)
                                .width(Length::Fill),
                            text_input("Hostname", &state.editing_hostname)
                                .on_input(Message::EditHostnameChanged)
                                .on_submit(Message::SaveEditedEntry)
                                .width(Length::Fill),
                            button("Salva").on_press(Message::SaveEditedEntry),
                            button("Annulla").on_press(Message::CancelEdit),
                        ]
                            .spacing(5)
                            .padding(5)
                            .align_y(Alignment::Center)
                            .into()
                    } else {
                        // Il record localhost è gestito dall'app: non modificabile
                        let editable = !entry.is_localhost();

                        let mut label = format!("{:<15} {}", entry.ip, entry.names());
                        if let Some(comment) = &entry.comment {
                            label.push_str(&format!("  # {}", comment));
                        }
                        let mut label = text(label).width(Length::Fill);
                        if !entry.enabled {
                            label = label.color(Color::from_rgb(0.5, 0.5, 0.5));
                        }

                        row![
                            label,
                            button(if entry.enabled { "Disattiva" } else { "Attiva" })
                                .on_press_maybe(editable.then_some(Message::ToggleEntry(index))),
                            button("Modifica")
                                .on_press_maybe(editable.then_some(Message::EditEntry(index))),
                            button("Elimina")
                                .on_press_maybe(editable.then_some(Message::DeleteEntry(index))),
                        ]
                            .spacing(5)
                            .padding(5)
                            .align_y(Alignment::Center)
                            .into()
                    }
                },
                Line::Comment(comment) => text(comment).color(Color::from_rgb(0.5, 0.5, 0.5)).into(),
                Line::Empty => text("").into(),
            }
        })
        .collect();

    let results_label = if !is_searching {
        text("")
    } else if entries.is_empty() {
        text("Nessun record trovato.")
    } else {
        text(format!("{} record trovati", entries.len()))
    };

    let search_section = row![
        text_input("Cerca per hostname...", &state.search_query)
            .on_input(Message::SearchQueryChanged)
            .width(Length::Fill),
        button("Pulisci").on_press_maybe(is_searching.then_some(Message::ClearSearch)),
        results_label.size(14).color(Color::from_rgb(0.5, 0.5, 0.5)),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let scrollable_entries = scrollable(column(entries).padding(5).spacing(5)).height(Length::Fill).spacing(10);

    column![
        Space::with_height(10),
        profile_info_row,
        permission_warning(state),
        Space::with_height(20),
        add_host_section,
        Space::with_height(20),
        text("Record nel file hosts:").size(22).color(Color::from_rgb(0.1, 0.5, 0.8)),
        search_section,
        scrollable_entries,
    ]
        .spacing(15)
        .padding(20)
        .into()
}

// Avviso mostrato nella vista principale quando l'app non può scrivere il file hosts
fn permission_warning(state: &MyApp) -> Element<'_, Message> {
    let Some(status) = state.hosts_permission.filter(PermissionStatus::needs_setup) else {
        return Space::with_height(0).into();
    };

    let warning = if status.data_dir_writable {
        format!("Permessi mancanti: le modifiche non verranno scritte in {}.", privileges::HOSTS_PATH)
    } else {
        "Permessi mancanti: la cartella dei profili appartiene a root, le modifiche non verranno salvate.".to_string()
    };

    row![
        text(warning)
            .color(Color::from_rgb(0.9, 0.6, 0.2))
            .width(Length::Fill),
        button("Configura permessi").on_press(Message::InstallSudoersPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
}

fn theme(_state: &MyApp) -> Theme {
    Theme::TokyoNight
}


pub fn init_app() -> iced::Result {
    let initial_state = MyApp {
        input_text_dns: std::env::var("HOSTS_MANAGER_DNS")
            .unwrap_or_else(|_| DEFAULT_DNS_SERVER.to_string()),
        ..Default::default()
    };

    iced::application("Hosts manager", update, view)
        .theme(theme)
        .settings(Settings {
            ..Default::default()
        })
        .run_with(|| {
            (initial_state, Task::batch([
                Task::done(Message::LoadProfiles),
                Task::done(Message::CheckHostsPermission),
            ]))
        })

}
