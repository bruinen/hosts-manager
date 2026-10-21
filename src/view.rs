// Interfaccia: barra laterale con i profili e area principale con i record del profilo attivo.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{
    button, center, checkbox, column, container, horizontal_rule, row, scrollable, text,
    text_input, toggler, Space,
};
use iced::{Alignment, Element, Length, Padding, Theme};

use crate::app::{AddMode, Message, MyApp};
use crate::db_manager::Profile;
use crate::host_manager::{hostname_matches, Entry, Line};
use crate::privileges::HostsPermission;
use crate::style;

pub fn view(state: &MyApp) -> Element<'_, Message> {
    row![sidebar(state), main_area(state)]
        .height(Length::Fill)
        .into()
}

// --- Barra laterale ---

fn sidebar(state: &MyApp) -> Element<'_, Message> {
    let title = column![
        text("Hosts Manager").size(18).font(style::SEMIBOLD),
        text("Profili del file hosts").size(12).style(style::muted_text),
    ]
        .spacing(2);

    let profiles = column(state.profiles.iter().map(|p| profile_item(state, p))).spacing(2);

    let new_profile = row![
        text_input("Nuovo profilo…", &state.new_profile_name)
            .on_input(Message::NewProfileNameChanged)
            .on_submit(Message::CreateProfileButtonPressed)
            .padding(8)
            .size(13)
            .style(style::input),
        button(text("+").size(16).font(style::SEMIBOLD))
            .on_press(Message::CreateProfileButtonPressed)
            .padding([4, 12])
            .style(style::primary),
    ]
        .spacing(6)
        .align_y(Alignment::Center);

    let actions = column![
        sidebar_action("Importa profilo…", Message::ImportProfilesButtonPressed),
        sidebar_action("Esporta profilo attivo…", Message::ExportProfilesButtonPressed),
    ]
        .spacing(2);

    let content = column![
        title,
        Space::with_height(12),
        section_label("PROFILI"),
        scrollable(profiles).height(Length::Fill),
        new_profile,
        horizontal_rule(1).style(style::separator),
        actions,
        permissions_panel(state),
    ]
        .spacing(10)
        .padding(16);

    container(content)
        .width(250)
        .height(Length::Fill)
        .style(style::sidebar)
        .into()
}

fn section_label(label: &str) -> Element<'_, Message> {
    text(label).size(11).font(style::SEMIBOLD).style(style::muted_text).into()
}

fn sidebar_action(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .on_press(message)
        .width(Length::Fill)
        .padding([7, 10])
        .style(style::ghost)
        .into()
}

fn profile_item<'a>(state: &'a MyApp, profile: &'a Profile) -> Element<'a, Message> {
    let active = state.selected_profile_id.as_deref() == Some(profile.id.as_str());

    // Conferma prima di eliminare un profilo
    if state.confirm_delete_profile.as_deref() == Some(profile.id.as_str()) {
        return container(
            column![
                text(format!("Eliminare \"{}\"?", profile.name)).size(13),
                row![
                    button(text("Elimina").size(12))
                        .on_press(Message::DeleteProfile(profile.id.clone()))
                        .padding([4, 10])
                        .style(style::danger),
                    button(text("Annulla").size(12))
                        .on_press(Message::CancelDeleteProfile)
                        .padding([4, 10])
                        .style(style::ghost),
                ]
                    .spacing(6),
            ]
                .spacing(8),
        )
            .padding(10)
            .width(Length::Fill)
            .style(style::warning_card)
            .into();
    }

    let entries = profile.hosts.iter().filter(|l| matches!(l, Line::Entry(_))).count();
    let label = row![
        text(&profile.name).size(14).width(Length::Fill),
        text(entries.to_string()).size(12).style(style::muted_text),
    ]
        .spacing(8)
        .align_y(Alignment::Center);

    let item = button(label)
        .width(Length::Fill)
        .padding([8, 10])
        .style(style::profile_item(active))
        .on_press_maybe((!active).then(|| Message::ProfileSelected(profile.id.clone())));

    // Il profilo attivo e quello di default non si possono eliminare
    let delete: Element<Message> = if active || profile.is_default() {
        Space::with_width(28).into()
    } else {
        button(text("×").size(15))
            .on_press(Message::RequestDeleteProfile(profile.id.clone()))
            .padding([4, 9])
            .style(style::ghost_danger)
            .into()
    };

    row![item, delete].spacing(2).align_y(Alignment::Center).into()
}

fn permissions_panel(state: &MyApp) -> Element<'_, Message> {
    let Some(status) = state.hosts_permission else {
        return text("Verifica dei permessi…").size(12).style(style::muted_text).into();
    };

    if status.needs_setup() {
        let description = if status.data_dir_writable {
            "Serve una regola sudoers per scrivere il file hosts senza avviare l'app come amministratore."
        } else {
            "La cartella dei profili appartiene a root: le modifiche non possono essere salvate."
        };
        return container(
            column![
                text("Permessi richiesti").size(13).font(style::SEMIBOLD).style(style::warning_text),
                text(description).size(12),
                button(text("Configura permessi").size(13))
                    .on_press(Message::InstallSudoersPressed)
                    .width(Length::Fill)
                    .padding([6, 12])
                    .style(style::primary),
            ]
                .spacing(8),
        )
            .padding(12)
            .style(style::warning_card)
            .into();
    }

    let label = match status.hosts {
        HostsPermission::Writable => "File hosts scrivibile",
        _ => "Permessi sudo attivi",
    };
    let remove: Element<Message> = if status.hosts == HostsPermission::Sudo {
        button(text("Rimuovi").size(12))
            .on_press(Message::RemoveSudoersPressed)
            .padding([3, 8])
            .style(style::ghost)
            .into()
    } else {
        Space::with_width(0).into()
    };

    row![
        container(Space::new(8, 8)).style(|theme: &Theme| style::dot(style::success_color(theme))(theme)),
        text(label).size(12).style(style::muted_text).width(Length::Fill),
        remove,
    ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

// --- Area principale ---

fn main_area(state: &MyApp) -> Element<'_, Message> {
    let Some(profile) = state.selected_profile() else {
        return center(text("Caricamento dei profili…").style(style::muted_text)).into();
    };

    let total = profile.hosts.iter().filter(|l| matches!(l, Line::Entry(_))).count();
    let disabled = profile.hosts.iter().filter(|l| matches!(l, Line::Entry(e) if !e.enabled)).count();
    let mut summary = format!("{} record", total);
    if disabled > 0 {
        summary.push_str(&format!(" · {} disattivati", disabled));
    }

    let header = row![
        column![
            text(&profile.name).size(26).font(style::SEMIBOLD),
            text(summary).size(13).style(style::muted_text),
        ]
            .spacing(4)
            .width(Length::Fill),
        search_box(state),
    ]
        .spacing(16)
        .align_y(Alignment::Center);

    column![header, add_card(state), entries_card(state, profile), status_bar(state)]
        .spacing(18)
        .padding(Padding { top: 28.0, right: 32.0, bottom: 16.0, left: 32.0 })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn search_box(state: &MyApp) -> Element<'_, Message> {
    let is_searching = !state.search_query.is_empty();
    let clear: Element<Message> = if is_searching {
        button(text("×").size(15))
            .on_press(Message::ClearSearch)
            .padding([4, 9])
            .style(style::ghost)
            .into()
    } else {
        Space::with_width(0).into()
    };

    row![
        text_input("Cerca hostname…", &state.search_query)
            .on_input(Message::SearchQueryChanged)
            .padding(10)
            .size(14)
            .width(280)
            .style(style::input),
        clear,
    ]
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
}

fn add_card(state: &MyApp) -> Element<'_, Message> {
    let segment = |label: &'static str, mode: AddMode| {
        button(text(label).size(13))
            .on_press(Message::AddModeChanged(mode))
            .padding([5, 14])
            .style(style::segment(state.add_mode == mode))
    };
    let mode_switch = container(row![segment("Manuale", AddMode::Manual), segment("Da DNS", AddMode::Dns)].spacing(2))
        .padding(3)
        .style(style::segmented);

    let fields = match state.add_mode {
        AddMode::Manual => row![
            text_input("Indirizzo IP", &state.input_ip)
                .on_input(Message::InputIpChanged)
                .on_submit(Message::ManualAddButtonPressed)
                .font(style::MONO)
                .padding(10)
                .width(190)
                .style(style::input),
            text_input("Hostname (più nomi separati da spazio)", &state.input_hostname)
                .on_input(Message::InputHostnameChanged)
                .on_submit(Message::ManualAddButtonPressed)
                .padding(10)
                .width(Length::Fill)
                .style(style::input),
            button(text("Aggiungi").size(14))
                .on_press(Message::ManualAddButtonPressed)
                .padding([10, 18])
                .style(style::primary),
        ],
        AddMode::Dns => row![
            text_input("Hostname da risolvere (es: api.example.com)", &state.input_text)
                .on_input(Message::InputChanged)
                .on_submit(Message::DnsLookupButtonPressed)
                .padding(10)
                .width(Length::Fill)
                .style(style::input),
            text_input("Server DNS (vuoto = sistema)", &state.input_text_dns)
                .on_input(Message::InputDNSChanged)
                .on_submit(Message::DnsLookupButtonPressed)
                .font(style::MONO)
                .padding(10)
                .width(230)
                .style(style::input),
            button(text("Risolvi e aggiungi").size(14))
                .on_press(Message::DnsLookupButtonPressed)
                .padding([10, 18])
                .style(style::primary),
        ],
    }
        .spacing(10)
        .align_y(Alignment::Center);

    container(
        column![
            row![
                text("Aggiungi record").size(15).font(style::SEMIBOLD).width(Length::Fill),
                mode_switch,
            ]
                .align_y(Alignment::Center),
            fields,
        ]
            .spacing(14),
    )
        .padding(18)
        .style(style::card)
        .into()
}

fn entries_card<'a>(state: &'a MyApp, profile: &'a Profile) -> Element<'a, Message> {
    let is_searching = !state.search_query.trim().is_empty();

    // Durante la ricerca mostra solo i record corrispondenti; commenti e righe vuote
    // solo se richiesti. L'indice resta quello originale, così le azioni agiscono sul record giusto.
    let visible: Vec<(usize, &Line)> = profile.hosts
        .iter()
        .enumerate()
        .filter(|(_, line)| match line {
            Line::Entry(entry) => hostname_matches(entry, &state.search_query),
            _ => state.show_comments && !is_searching,
        })
        .collect();
    let matches = visible.iter().filter(|(_, l)| matches!(l, Line::Entry(_))).count();

    let results: Element<Message> = if is_searching {
        text(format!("{} risultati", matches)).size(13).style(style::muted_text).into()
    } else {
        Space::with_width(0).into()
    };

    let header = row![
        text("Record").size(15).font(style::SEMIBOLD).width(Length::Fill),
        results,
        checkbox("Mostra commenti", state.show_comments)
            .on_toggle(Message::ShowCommentsToggled)
            .size(15)
            .text_size(13),
    ]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding([14, 18]);

    let columns = row![
        Space::with_width(40),
        text("INDIRIZZO IP").size(11).font(style::SEMIBOLD).style(style::muted_text).width(170),
        text("HOSTNAME").size(11).font(style::SEMIBOLD).style(style::muted_text).width(Length::Fill),
    ]
        .spacing(16)
        .padding([0, 18]);

    let body: Element<Message> = if visible.is_empty() {
        let message = if is_searching {
            format!("Nessun record corrisponde a \"{}\".", state.search_query.trim())
        } else {
            "Nessun record. Aggiungine uno dal riquadro qui sopra.".to_string()
        };
        center(text(message).size(14).style(style::muted_text)).into()
    } else {
        let mut rows = column![].padding([4, 0]);
        for (position, (index, line)) in visible.into_iter().enumerate() {
            if position > 0 {
                rows = rows.push(container(horizontal_rule(1).style(style::separator)).padding([0, 18]));
            }
            rows = rows.push(match line {
                Line::Entry(entry) => entry_row(state, index, entry),
                Line::Comment(comment) => container(text(comment).font(style::MONO).size(12).style(style::muted_text))
                    .padding([6, 18])
                    .into(),
                Line::Empty => Space::with_height(4).into(),
            });
        }
        scrollable(rows)
            .direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(3)))
            .height(Length::Fill)
            .into()
    };

    container(
        column![header, columns, horizontal_rule(1).style(style::separator), body]
            .spacing(8),
    )
        .height(Length::Fill)
        .style(style::card)
        .into()
}

fn entry_row<'a>(state: &'a MyApp, index: usize, entry: &'a Entry) -> Element<'a, Message> {
    if state.editing_index == Some(index) {
        return row![
            Space::with_width(40),
            text_input("Indirizzo IP", &state.editing_ip)
                .on_input(Message::EditIpChanged)
                .on_submit(Message::SaveEditedEntry)
                .font(style::MONO)
                .padding(8)
                .width(170)
                .style(style::input),
            text_input("Hostname", &state.editing_hostname)
                .on_input(Message::EditHostnameChanged)
                .on_submit(Message::SaveEditedEntry)
                .padding(8)
                .width(Length::Fill)
                .style(style::input),
            button(text("Salva").size(13))
                .on_press(Message::SaveEditedEntry)
                .padding([6, 14])
                .style(style::primary),
            button(text("Annulla").size(13))
                .on_press(Message::CancelEdit)
                .padding([6, 12])
                .style(style::ghost),
        ]
            .spacing(16)
            .padding([8, 18])
            .align_y(Alignment::Center)
            .into();
    }

    // Il record localhost è gestito dall'app: non modificabile
    let is_system = entry.is_localhost();
    let text_style: fn(&Theme) -> text::Style = if entry.enabled { style::normal_text } else { style::muted_text };

    let switch = toggler(entry.enabled)
        .on_toggle_maybe((!is_system).then_some(move |_| Message::ToggleEntry(index)))
        .size(18)
        .style(style::switch);

    let mut names = column![text(entry.names()).size(14).style(text_style)].spacing(2);
    if let Some(comment) = &entry.comment {
        names = names.push(text(comment).size(12).style(style::muted_text));
    }

    let actions: Element<Message> = if is_system {
        container(text("di sistema").size(11))
            .padding([3, 10])
            .style(style::badge)
            .into()
    } else {
        row![
            button(text("Modifica").size(13))
                .on_press(Message::EditEntry(index))
                .padding([5, 10])
                .style(style::ghost),
            button(text("Elimina").size(13))
                .on_press(Message::DeleteEntry(index))
                .padding([5, 10])
                .style(style::ghost_danger),
        ]
            .spacing(2)
            .into()
    };

    row![
        container(switch).width(40),
        text(&entry.ip).font(style::MONO).size(13).style(text_style).width(170),
        names.width(Length::Fill),
        actions,
    ]
        .spacing(16)
        .padding([9, 18])
        .align_y(Alignment::Center)
        .into()
}

// Messaggio di stato in basso (verde per il successo, rosso per gli errori)
fn status_bar(state: &MyApp) -> Element<'_, Message> {
    let toast: Element<Message> = if let Some(msg) = &state.error_message {
        container(text(msg).size(13)).padding([7, 14]).style(style::toast_error).into()
    } else if let Some(msg) = &state.success_message {
        container(text(msg).size(13)).padding([7, 14]).style(style::toast_success).into()
    } else {
        Space::with_height(0).into()
    };

    container(toast).height(34).align_y(Alignment::Center).into()
}
