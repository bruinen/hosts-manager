use iced::{Alignment, Color, Element, Length, widget::{column, text, button, text_input, row, scrollable, container, Space}};
use crate::app::{status_label, Message, MyApp};
use crate::privileges::{HostsPermission, SUDOERS_FILE};


pub fn view(state: &MyApp) -> Element<'_, Message> {
    let new_profile_input = row![
        text_input("Nuovo profilo", &state.new_profile_name)
            .on_input(Message::NewProfileNameChanged)
            .on_submit(Message::CreateProfileButtonPressed)
            .width(Length::Fill),
        button("Crea").on_press(Message::CreateProfileButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let profiles_list: Vec<Element<Message>> = state.profiles
        .iter()
        .map(|profile| {
            let is_selected = state.selected_profile_id.as_ref() == Some(&profile.id);

            let select_button = if is_selected {
                button("Attivo")
            } else {
                button("Seleziona").on_press(Message::ProfileSelected(profile.id.clone()))
            };

            let delete_button = if profile.is_default() {
                button("Default")
            } else {
                button("Elimina").on_press_maybe((!is_selected).then(|| Message::DeleteProfile(profile.id.clone())))
            };

            row![
                text(&profile.name).width(Length::Fill),
                select_button,
                delete_button,
            ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        })
        .collect();

    let scrollable_profiles = scrollable(column(profiles_list).spacing(5)).height(Length::FillPortion(1));

    let import_export_row = row![
        button("Esporta profilo selezionato").on_press(Message::ExportProfilesButtonPressed),
        button("Importa profilo").on_press(Message::ImportProfilesButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let (permission_text, permission_color) = match state.hosts_permission {
        Some(HostsPermission::Writable) => ("Il file hosts è scrivibile direttamente.".to_string(), Color::from_rgb(0.2, 0.7, 0.2)),
        Some(HostsPermission::Sudo) => (format!("Scrittura tramite sudo attiva ({}).", SUDOERS_FILE), Color::from_rgb(0.2, 0.7, 0.2)),
        Some(HostsPermission::Missing) => ("Permessi non configurati: il file hosts non può essere modificato.".to_string(), Color::from_rgb(0.9, 0.6, 0.2)),
        None => ("Verifica dei permessi in corso...".to_string(), Color::from_rgb(0.5, 0.5, 0.5)),
    };

    let permission_row = row![
        text(permission_text).color(permission_color).width(Length::Fill),
        button("Configura permessi")
            .on_press_maybe((state.hosts_permission == Some(HostsPermission::Missing)).then_some(Message::InstallSudoersPressed)),
        button("Rimuovi permessi")
            .on_press_maybe((state.hosts_permission == Some(HostsPermission::Sudo)).then_some(Message::RemoveSudoersPressed)),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let content = column![
        text("Gestione dei Profili").size(30).color(Color::from_rgb(0.1, 0.5, 0.8)),
        Space::with_height(20),
        new_profile_input,
        Space::with_height(20),
        status_label(state),
        text("Profili esistenti:").size(20),
        scrollable_profiles,
        Space::with_height(20),
        import_export_row,
        Space::with_height(20),
        text("Permessi file hosts:").size(20),
        permission_row,
        Space::with_height(20),
        button("Torna alla vista principale").on_press(Message::ShowMainView)
    ]
        .spacing(15)
        .padding(20)
        .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
