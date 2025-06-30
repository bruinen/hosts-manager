use iced::{Alignment, Color, Element, Length, Task, Theme,
           widget::{column, Text,text, button, text_input, row, scrollable, container, Space}, Settings, Renderer};
use crate::host_manager::{Line, write_hosts_entries_to_file, Entry};
use crate::{host_manager, profile_view,db_manager};
use crate::db_manager::{update_profile, Profile};
use crate::dns_lookup::resolve_hostname_with_specific_dns;

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
    DnsLookupResult(Result<String, String>),

    DeleteEntry(usize),
    EditEntry(usize),
    EditIpChanged(String),
    EditHostnameChanged(String),
    SaveEditedEntry,
    CancelEdit,
    SaveSuccess,
    SaveError(String),
    ProfileSelected(Profile),
    NewProfileNameChanged(String),
    CreateProfileButtonPressed,
    LoadProfiles,
    LoadProfilesResult(Result<Vec<Profile>, String>),
    ShowMainView,
    ShowProfilesView,
    DeleteProfile(String),
    UpdateDatabaseResult(Result<(), String>),

    ExportProfilesButtonPressed,
    ImportProfilesButtonPressed,
    ExportProfilesResult(Result<(), String>),
    ImportProfilesResult(Result<(), String>),
}

#[derive(Debug, Default)]
pub struct MyApp {
    pub input_text: String,
    pub input_text_dns: String,
    pub input_ip: String,
    pub input_hostname: String,
    pub file_lines: Vec<Line>,
    pub editing_index: Option<usize>,
    pub editing_ip: String,
    pub editing_hostname: String,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub profiles: Vec<Profile>,
    pub selected_profile: Option<Profile>,
    pub new_profile_name: String,
    pub view: View,

}



fn update(state: &mut MyApp, message: Message) -> Task<Message> {
    if !matches!(message, Message::SaveSuccess | Message::SaveError(_)) {
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
            let hostname = state.input_hostname.clone();
            let ip = state.input_ip.clone();

            if hostname.is_empty() || ip.is_empty() {
                state.error_message = Some("IP e Hostname are required.".to_string());
                return Task::none();
            }

            state.input_ip.clear();
            state.input_hostname.clear();

            let new_entry = Line::Entry(Entry {
                ip,
                hostname,
                enabled:true,
                comment: None,
            });
            state.file_lines.push(new_entry);

            if let Some(profile) = &mut state.selected_profile {
                profile.hosts = state.file_lines.clone();
                let entries_to_save = state.file_lines.clone();
                let profile_to_update = profile.clone();

                return Task::batch(vec![
                    Task::perform(async move {
                        write_hosts_entries_to_file(&entries_to_save)
                            .map_err(|e| e.to_string())
                    }, |result| {
                        match result {
                            Ok(_) => Message::SaveSuccess,
                            Err(e) => Message::SaveError(e),
                        }
                    }),
                    Task::perform(async move {
                        let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                        update_profile(&conn, &profile_to_update)
                            .map_err(|e| e.to_string())
                    }, Message::UpdateDatabaseResult),
                ]);
            } else {
                state.error_message = Some("No Profile Selected.".to_string());
            }


        }
        Message::DnsLookupButtonPressed => {
            let hostname = state.input_text.clone();
            let dns_server = state.input_text_dns.clone();
            if hostname.is_empty() {
                state.error_message = Some("Hostname is required for DNS lookup.".to_string());
                return Task::none();
            }
            state.success_message = Some("IP searching ...".to_string());

            return Task::perform(async move {
                resolve_hostname_with_specific_dns(&hostname,&dns_server)
                    .map_err(|e| e.to_string())
            }, Message::DnsLookupResult);
        }
        Message::DnsLookupResult(Ok(ip_address)) => {
            state.success_message = Some(format!("IP found: {}", ip_address));

            let new_entry = Line::Entry(Entry {
                ip: ip_address,
                hostname: state.input_text.clone(),
                enabled: true,
                comment: None,
            });
            state.file_lines.push(new_entry);
            if let Some(profile) = &mut state.selected_profile {
                profile.hosts = state.file_lines.clone();
                let entries_to_save = state.file_lines.clone();
                let profile_to_update = profile.clone();

                return Task::batch(vec![
                    Task::perform(async move {
                        write_hosts_entries_to_file(&entries_to_save)
                            .map_err(|e| e.to_string())
                    }, |result| {
                        match result {
                            Ok(_) => Message::SaveSuccess,
                            Err(e) => Message::SaveError(e),
                        }
                    }),
                    Task::perform(async move {
                        let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                        update_profile(&conn, &profile_to_update)
                            .map_err(|e| e.to_string())
                    }, Message::UpdateDatabaseResult),
                ]);
            } else {
                state.error_message = Some("Profile is required.".to_string());
            }
            state.input_text.clear();

        }

        Message::DnsLookupResult(Err(e)) => {
            state.error_message = Some(e);
        }

        Message::DeleteEntry(index) => {
            if index < state.file_lines.len() {
                state.file_lines.remove(index);
                if let Some(profile) = &mut state.selected_profile {
                    profile.hosts = state.file_lines.clone();

                    let entries_to_save = state.file_lines.clone();
                    let profile_to_update = profile.clone();

                    return Task::batch(vec![
                        Task::perform(async move {
                            write_hosts_entries_to_file(&entries_to_save)
                                .map_err(|e| e.to_string())
                        }, |result| {
                            match result {
                                Ok(_) => Message::SaveSuccess,
                                Err(e) => Message::SaveError(e),
                            }
                        }),
                        Task::perform(async move {
                            let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                            update_profile(&conn, &profile_to_update)
                                .map_err(|e| e.to_string())
                        }, Message::UpdateDatabaseResult),
                    ]);
                }
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
                    state.editing_ip.clear();
                    state.editing_hostname.clear();

                    if let Some(profile) = &mut state.selected_profile {
                        profile.hosts = state.file_lines.clone();

                        let entries_to_save = state.file_lines.clone();
                        let profile_to_update = profile.clone();

                        return Task::batch(vec![
                            Task::perform(async move {
                                write_hosts_entries_to_file(&entries_to_save)
                                    .map_err(|e| e.to_string())
                            }, |result| {
                                match result {
                                    Ok(_) => Message::SaveSuccess,
                                    Err(e) => Message::SaveError(e),
                                }
                            }),
                            Task::perform(async move {
                                let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                                update_profile(&conn, &profile_to_update)
                                    .map_err(|e| e.to_string())
                            }, Message::UpdateDatabaseResult),
                        ]);
                    } else {
                        state.error_message = Some("Select a profile to save changes.".to_string());
                    }
                }
            }
        }

        Message::CancelEdit => {
            state.editing_index = None;
            state.editing_ip = String::new();
            state.editing_hostname = String::new();
            println!("Modifica annullata.");
        }
        Message::SaveSuccess => {
            state.error_message = None;
            state.success_message = Some(String::from("Saved successfully."));
        }
        Message::SaveError(e) => {
            state.error_message = Some(e);
        }
        Message::LoadProfiles => {
            return Task::perform(async {
                let conn = match db_manager::initialize_db() {
                    Ok(c) => c,
                    Err(e) => return Err(format!("Errore nell'inizializzazione del database: {}", e)),
                };
                let profiles = db_manager::get_all_profiles(&conn)
                    .map_err(|e| format!("Errore nel caricamento dei profili: {}", e))?;

                Ok(profiles)
            }, Message::LoadProfilesResult);
        }
        Message::LoadProfilesResult(Ok(profiles)) => {
            if profiles.is_empty() {

                state.success_message = Some("Creating  'Default' profile ...".to_string());
                return Task::perform(async {
                    let hosts = host_manager::load_hosts_entries();
                    let conn = db_manager::initialize_db()?;
                    db_manager::create_profile(&conn, "Default", &hosts)?;
                    Ok(())
                }, |result: Result<(), rusqlite::Error>| {
                    match result {
                        Ok(_) => Message::LoadProfiles,
                        Err(e) => Message::SaveError(format!("Error creating default profile: {}", e)),
                    }
                });
            } else {
                state.profiles = profiles;


                let active_profile = state.profiles.iter().find(|p| p.is_active).cloned();

                if let Some(profile) = active_profile {
                    state.selected_profile = Some(profile.clone());
                    state.file_lines = profile.hosts.clone();
                    state.success_message = Some(format!("Profile loaded: {}", profile.name));
                } else {
                    state.selected_profile = state.profiles.first().cloned();
                    if let Some(profile) = &state.selected_profile {
                        state.file_lines = profile.hosts.clone();
                        state.success_message = Some(format!("Default profile loaded: {}", profile.name));
                    }
                }
            }
        }
        Message::LoadProfilesResult(Err(e)) => {
            state.error_message = Some(e);
        }

        Message::ProfileSelected(profile) => {
            state.selected_profile = Some(profile.clone());

            state.file_lines = profile.hosts.clone();

            let entries_to_save = state.file_lines.clone();
            let profile_id_to_activate = profile.id.clone();

            return Task::batch(vec![
                Task::perform(async move {
                    write_hosts_entries_to_file(&entries_to_save)
                        .map_err(|e| e.to_string())
                }, |result| {
                    match result {
                        Ok(_) => Message::SaveSuccess,
                        Err(e) => Message::SaveError(e),
                    }
                }),
                Task::perform(async move {
                    let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                    db_manager::set_active_profile(&conn, &profile_id_to_activate)
                        .map_err(|e| e.to_string())
                }, |result| {
                    match result {
                        Ok(_) => Message::LoadProfiles,
                        Err(e) => Message::SaveError(format!("Errore nell'attivazione del profilo: {}", e)),
                    }
                })
            ]);
        }
        Message::NewProfileNameChanged(name) => {
            state.new_profile_name = name;
        }
        Message::CreateProfileButtonPressed => {
            let name = state.new_profile_name.clone();
            if name.is_empty() {
                state.error_message = Some("Il nome del profilo non può essere vuoto.".to_string());
                return Task::none();
            }


            let mut current_hosts =Vec::new();

            let localhost_entry = Line::Entry(Entry {
                ip: "127.0.0.1".to_string(),
                hostname: "localhost".to_string(),
                enabled: true,
                comment: None,
            });

            if !current_hosts.contains(&localhost_entry) {
                current_hosts.insert(0, localhost_entry);
            }

            return Task::perform(async move {
                let conn = db_manager::initialize_db()?;
                db_manager::create_profile(&conn, &name, &current_hosts)?;
                Ok(())
            }, |result: Result<(), rusqlite::Error>| {
                match result {
                    Ok(_) => Message::LoadProfiles,
                    Err(e) => Message::SaveError(format!("Errore nella creazione del profilo: {}", e)),
                }
            });
        }
        Message::ShowMainView => {
            state.view = View::Main;
        }
        Message::ShowProfilesView => {
            state.view = View::Profiles;
        }
        Message::DeleteProfile(profile_id) => {
            if let Some(selected_profile) = &state.selected_profile {
                if selected_profile.id == profile_id && selected_profile.name == "Default" {
                    state.error_message = Some("Impossibile eliminare il profilo 'Default'.".to_string());
                    return Task::none();
                }
            }

            if let Some(selected_profile) = &state.selected_profile {
                if selected_profile.id == profile_id {
                    state.error_message = Some("Impossibile eliminare il profilo attivo. Seleziona un altro profilo prima di procedere.".to_string());
                    return Task::none();
                }
            }

            state.success_message = Some("Eliminazione del profilo...".to_string());

            return Task::perform(async move {
                let conn = db_manager::initialize_db()?;
                db_manager::delete_profile(&conn, &profile_id)?;
                Ok(())
            }, |result: Result<(), rusqlite::Error>| {
                match result {
                    Ok(_) => Message::LoadProfiles,
                    Err(e) => Message::SaveError(format!("Errore nell'eliminazione del profilo: {}", e)),
                }
            });
        }
        Message::UpdateDatabaseResult(Ok(_)) => {
        }
        Message::UpdateDatabaseResult(Err(e)) => {
            state.error_message = Some(format!("Database update error: {}", e));
        }

        Message::ExportProfilesButtonPressed => {
            if let Some(profile) = &state.selected_profile {
                let profile_to_export = profile.clone();
                state.success_message = Some("Apertura finestra di dialogo...".to_string());

                return Task::perform(async move {
                    let file_path = rfd::AsyncFileDialog::new()
                        .add_filter("JSON Profile", &["json"])
                        .set_file_name(&format!("{}.json", profile_to_export.name))
                        .save_file()
                        .await;

                    if let Some(file) = file_path {
                        let json_data = serde_json::to_string_pretty(&profile_to_export)
                            .map_err(|e| format!("Errore di serializzazione: {}", e))?;

                        std::fs::write(file.path(), json_data)
                            .map_err(|e| format!("Errore di scrittura del file: {}", e))?;

                        Ok(())
                    } else {
                        Err("Operazione di esportazione annullata.".to_string())
                    }
                }, |result| Message::ExportProfilesResult(result));
            } else {
                state.error_message = Some("Seleziona un profilo da esportare.".to_string());
            }

        }
        Message::ImportProfilesButtonPressed => {
            state.success_message = Some("Apertura finestra di dialogo...".to_string());

            return Task::perform(async {
                let file_path = rfd::AsyncFileDialog::new()
                    .add_filter("JSON Profile", &["json"])
                    .pick_file()
                    .await;

                if let Some(file) = file_path {
                    let json_data = std::fs::read_to_string(file.path())
                        .map_err(|e| format!("Errore di lettura del file: {}", e))?;

                    let imported_profile: Profile = serde_json::from_str(&json_data)
                        .map_err(|e| format!("Errore di deserializzazione: {}", e))?;

                    let conn = db_manager::initialize_db().map_err(|e| e.to_string())?;
                    db_manager::import_profile(&conn, &imported_profile)
                        .map_err(|e| format!("Errore di importazione: {}", e))?;

                    Ok(())
                } else {
                    Err("Operazione di importazione annullata.".to_string())
                }
            }, |result| Message::ImportProfilesResult(result));
        }
        Message::ExportProfilesResult(Ok(_)) => {
            state.success_message = Some("Profilo esportato con successo!".to_string());
        }
        Message::ExportProfilesResult(Err(e)) => {
            state.error_message = Some(e);
        }

        Message::ImportProfilesResult(Ok(_)) => {
            state.success_message = Some("Profilo importato con successo!".to_string());
            let _ = Task::perform(async { Message::LoadProfiles }, |msg| msg);
        }
        Message::ImportProfilesResult(Err(e)) => {
            state.error_message = Some(e);
        }

    }

    Task::none()
}
pub fn view(state: &MyApp) -> Element<Message> {
    match state.view {
        View::Main => main_view(state),
        View::Profiles => profile_view::view(state),
    }
}


fn main_view(state: &MyApp) -> Element<Message> {


    let status_label: Text<'_, Theme, Renderer> = if let Some(msg) = &state.error_message {
        println!("{}", msg);
        text(msg).size(16).color(Color::from_rgb(0.8, 0.2, 0.2))
    } else if let Some(msg) = &state.success_message {
        println!("{}", msg);
        text(msg).size(16).color(Color::from_rgb(0.2, 0.7, 0.2))
    } else {
        text("")
    };

    let selected_profile_name = state.selected_profile
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "No Profile Selected".to_string());

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
        .width(Length::Fill),
         text_input(
            "DNS server (es:8.8.8.8)",
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
            "Hostname (es: server.local)",
            &state.input_hostname,
        )
        .on_input(Message::InputHostnameChanged)
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
            status_label,
        ]
            .spacing(10)
    )
        .padding(15)
        .style(container::rounded_box);

    let entries: Vec<Element<Message>> = state.file_lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            match line {
                Line::Entry(entry) => {
                    if state.editing_index == Some(index) {
                        row![
                            text_input("IP", &state.editing_ip)
                                .on_input(Message::EditIpChanged)
                                .width(Length::Fill),
                            text_input("Hostname", &state.editing_hostname)
                                .on_input(Message::EditHostnameChanged)
                                .width(Length::Fill),
                            button("Salva").on_press(Message::SaveEditedEntry),
                            button("Annulla").on_press(Message::CancelEdit),
                        ]
                            .spacing(5)
                            .padding(5)
                            .align_y(Alignment::Center)
                            .into()
                    } else {
                        let is_localhost = entry.ip == "127.0.0.1" && entry.hostname == "localhost";
                        let mut delete_button  =  button("Elimina").on_press(Message::DeleteEntry(index)).into();
                        let mut modify_button =  button("Modifica").on_press(Message::EditEntry(index));

                        if is_localhost {
                            delete_button= button("Elimina");
                            modify_button= button("Modifica");
                        }
                        
                        row![
                            text(format!("{:<15} {}", entry.ip, entry.hostname))
                                .width(Length::Fill),
                            modify_button,
                            delete_button,
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

    let scrollable_entries = scrollable(column(entries).padding(5).spacing(5)).height(Length::Fill).spacing(10);

    let content = column![
        Space::with_height(10),
        profile_info_row,
        Space::with_height(20),
        add_host_section,
        Space::with_height(20),
        text("Record nel file hosts:").size(22).color(Color::from_rgb(0.1, 0.5, 0.8)),
        scrollable_entries,
    ]
        .spacing(15)
        .padding(20);

    content.into()
}

fn theme(_state: &MyApp) -> Theme {
    Theme::TokyoNight
}


pub fn init_app() -> iced::Result {
    let initial_state = MyApp {
        input_text: String::new(),
        input_text_dns: "10.10.10.10".to_string(),
        input_ip: String::new(),
        input_hostname: String::new(),
        file_lines: Vec::new(),
        editing_index: None,
        editing_ip: String::new(),
        editing_hostname: String::new(),
        error_message: None,
        success_message: None,
        profiles: Vec::new(),
        selected_profile: None,
        new_profile_name: String::new(),
        view: View::Main,
    };


    iced::application("Hosts manager", update, view)
        .theme(theme)
        .settings(Settings {
            ..Default::default()
        })
        .run_with(|| {
            (initial_state, Task::perform(async {}, |_| Message::LoadProfiles))
        })

}

