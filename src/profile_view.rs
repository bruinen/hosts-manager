
use iced::{Alignment, Color, Element, Length, widget::{column, text, button, text_input, row, scrollable, container, Space}, Theme, Renderer};
use iced::widget::{Row,Text};
use crate::app::{Message, MyApp};


pub fn view(state: &MyApp) -> Element<Message> {
    // Label per mostrare l'errore o il successo
    let status_label: Text<'_, Theme, Renderer> = if let Some(msg) = &state.error_message {
        text(msg).size(16).color(Color::from_rgb(0.8, 0.2, 0.2)) // Rosso per gli errori
    } else if let Some(msg) = &state.success_message {
        text(msg).size(16).color(Color::from_rgb(0.2, 0.7, 0.2)) // Verde per il successo
    } else {
        text("")
    };
    // Vista per la creazione di un nuovo profilo
    let new_profile_input = row![
        text_input("Nuovo nome profilo", &state.new_profile_name)
            .on_input(Message::NewProfileNameChanged)
            .width(Length::Fill),
        button("Crea").on_press(Message::CreateProfileButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    // Lista dei profili esistenti
    let profiles_list: Vec<Element<Message>> = state.profiles
        .iter()
        .map(|profile| {
            let select_button = if state.selected_profile.as_ref().map(|p| p.id.clone()) == Some(profile.id.clone()) {
                // Il profilo è già selezionato, mostra un pulsante diverso o un testo
                button("Attivo")
            } else {
                button("Seleziona").on_press(Message::ProfileSelected(profile.clone())).into()
            };

            // Aggiungi una condizione per il pulsante "Elimina"
            let delete_button = if profile.name == "Default" {
                // Non mostrare il pulsante "Elimina" per il profilo "Default"
                button("Default")
            } else {
                button("Elimina").on_press(Message::DeleteProfile(profile.id.clone())).into()
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

    let import_export_row: Row<'_, _, Theme, Renderer> = row![
        button("Esporta profilo selezionato").on_press(Message::ExportProfilesButtonPressed),
        button("Importa profilo").on_press(Message::ImportProfilesButtonPressed),
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let content = column![
        text("Gestione dei Profili").size(30).color(Color::from_rgb(0.1, 0.5, 0.8)),
        Space::with_height(20),
        new_profile_input,
        Space::with_height(20),
        status_label,
        text("Profili esistenti:").size(20),
        scrollable_profiles,
        Space::with_height(20),
        import_export_row,
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