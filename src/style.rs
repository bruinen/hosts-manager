// Tema e stili dell'interfaccia: palette chiara/scura (segue il tema di sistema),
// superfici con angoli arrotondati e pulsanti/campi in stile moderno.

use iced::theme::Palette;
use iced::widget::{button, container, rule, text, text_input, toggler};
use iced::{border, font, Background, Border, Color, Font, Shadow, Theme, Vector};

const RADIUS: f32 = 8.0;
const CARD_RADIUS: f32 = 12.0;

#[cfg(target_os = "macos")]
pub const FONT: Font = Font::with_name("Helvetica Neue");
#[cfg(not(target_os = "macos"))]
pub const FONT: Font = Font::DEFAULT;

#[cfg(target_os = "macos")]
pub const MONO: Font = Font::with_name("Menlo");
#[cfg(not(target_os = "macos"))]
pub const MONO: Font = Font::MONOSPACE;

pub const SEMIBOLD: Font = Font { weight: font::Weight::Semibold, ..FONT };

// Tema dell'app: variante chiara o scura in base all'aspetto di sistema
pub fn app_theme() -> Theme {
    if matches!(Theme::default(), Theme::Dark) {
        Theme::custom("Hosts Manager Scuro".to_string(), Palette {
            background: Color::from_rgb8(0x16, 0x18, 0x1D),
            text: Color::from_rgb8(0xE6, 0xE8, 0xEB),
            primary: Color::from_rgb8(0x4C, 0x8D, 0xFF),
            success: Color::from_rgb8(0x3F, 0xB9, 0x50),
            danger: Color::from_rgb8(0xF8, 0x51, 0x49),
        })
    } else {
        Theme::custom("Hosts Manager Chiaro".to_string(), Palette {
            background: Color::from_rgb8(0xF5, 0xF6, 0xF8),
            text: Color::from_rgb8(0x1F, 0x23, 0x28),
            primary: Color::from_rgb8(0x25, 0x63, 0xEB),
            success: Color::from_rgb8(0x16, 0xA3, 0x4A),
            danger: Color::from_rgb8(0xDC, 0x26, 0x26),
        })
    }
}

// Toni di superficie non coperti dalla palette di iced
struct Tones {
    sidebar: Color,
    card: Color,
    border: Color,
    hover: Color,
    muted: Color,
    warning: Color,
}

fn tones(theme: &Theme) -> Tones {
    if theme.extended_palette().is_dark {
        Tones {
            sidebar: Color::from_rgb8(0x11, 0x13, 0x17),
            card: Color::from_rgb8(0x1E, 0x21, 0x28),
            border: Color::from_rgb8(0x2C, 0x30, 0x38),
            hover: Color::from_rgb8(0x27, 0x2B, 0x33),
            muted: Color::from_rgb8(0x8B, 0x93, 0x9E),
            warning: Color::from_rgb8(0xE3, 0xA0, 0x08),
        }
    } else {
        Tones {
            sidebar: Color::from_rgb8(0xEC, 0xEE, 0xF2),
            card: Color::WHITE,
            border: Color::from_rgb8(0xE1, 0xE4, 0xE8),
            hover: Color::from_rgb8(0xF0, 0xF2, 0xF5),
            muted: Color::from_rgb8(0x6B, 0x72, 0x80),
            warning: Color::from_rgb8(0xB4, 0x53, 0x09),
        }
    }
}

fn soft(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

fn rounded(radius: f32) -> Border {
    Border { radius: radius.into(), ..Border::default() }
}

// --- Testo ---

pub fn muted_text(theme: &Theme) -> text::Style {
    text::Style { color: Some(tones(theme).muted) }
}

pub fn normal_text(_theme: &Theme) -> text::Style {
    text::Style { color: None }
}

pub fn warning_text(theme: &Theme) -> text::Style {
    text::Style { color: Some(tones(theme).warning) }
}

// --- Contenitori ---

pub fn sidebar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tones(theme).sidebar.into()),
        ..container::Style::default()
    }
}

pub fn card(theme: &Theme) -> container::Style {
    let tones = tones(theme);
    container::Style {
        background: Some(tones.card.into()),
        border: Border { color: tones.border, width: 1.0, radius: CARD_RADIUS.into() },
        shadow: Shadow {
            color: Color { a: 0.04, ..Color::BLACK },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..container::Style::default()
    }
}

pub fn warning_card(theme: &Theme) -> container::Style {
    let warning = tones(theme).warning;
    container::Style {
        background: Some(soft(warning, 0.10).into()),
        border: Border { color: soft(warning, 0.35), width: 1.0, radius: RADIUS.into() },
        ..container::Style::default()
    }
}

pub fn badge(theme: &Theme) -> container::Style {
    let tones = tones(theme);
    container::Style {
        background: Some(tones.hover.into()),
        text_color: Some(tones.muted),
        border: rounded(999.0),
        ..container::Style::default()
    }
}

pub fn dot(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: rounded(999.0),
        ..container::Style::default()
    }
}

pub fn success_color(theme: &Theme) -> Color {
    theme.palette().success
}

pub fn toast_success(theme: &Theme) -> container::Style {
    let color = theme.palette().success;
    container::Style {
        background: Some(soft(color, 0.12).into()),
        text_color: Some(color),
        border: rounded(999.0),
        ..container::Style::default()
    }
}

pub fn toast_error(theme: &Theme) -> container::Style {
    let color = theme.palette().danger;
    container::Style {
        background: Some(soft(color, 0.12).into()),
        text_color: Some(color),
        border: rounded(999.0),
        ..container::Style::default()
    }
}

pub fn segmented(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tones(theme).hover.into()),
        border: rounded(RADIUS),
        ..container::Style::default()
    }
}

pub fn separator(theme: &Theme) -> rule::Style {
    rule::Style {
        color: tones(theme).border,
        width: 1,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
    }
}

// --- Pulsanti ---

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    button::Style { border: rounded(RADIUS), ..button::primary(theme, status) }
}

pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    button::Style { border: rounded(RADIUS), ..button::danger(theme, status) }
}

// Pulsante senza sfondo, evidenziato al passaggio del mouse
pub fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let tones = tones(theme);
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => (Some(Background::from(tones.hover)), theme.palette().text),
        button::Status::Disabled => (None, soft(tones.muted, 0.5)),
        button::Status::Active => (None, tones.muted),
    };
    button::Style { background, text_color, border: rounded(RADIUS), shadow: Shadow::default() }
}

pub fn ghost_danger(theme: &Theme, status: button::Status) -> button::Style {
    let danger = theme.palette().danger;
    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(soft(danger, 0.10).into()),
            text_color: danger,
            border: rounded(RADIUS),
            shadow: Shadow::default(),
        },
        _ => ghost(theme, status),
    }
}

// Voce della lista profili: evidenziata quando è il profilo attivo
pub fn profile_item(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let primary = theme.palette().primary;
        if active {
            return button::Style {
                background: Some(soft(primary, 0.14).into()),
                text_color: primary,
                border: rounded(RADIUS),
                shadow: Shadow::default(),
            };
        }
        let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| Background::from(tones(theme).hover));
        button::Style {
            background,
            text_color: theme.palette().text,
            border: rounded(RADIUS),
            shadow: Shadow::default(),
        }
    }
}

// Segmento del selettore Manuale / DNS
pub fn segment(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let tones = tones(theme);
        if active {
            button::Style {
                background: Some(tones.card.into()),
                text_color: theme.palette().text,
                border: rounded(RADIUS - 2.0),
                shadow: Shadow {
                    color: Color { a: 0.08, ..Color::BLACK },
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                },
            }
        } else {
            button::Style {
                background: None,
                text_color: if status == button::Status::Hovered { theme.palette().text } else { tones.muted },
                border: rounded(RADIUS - 2.0),
                shadow: Shadow::default(),
            }
        }
    }
}

// Interruttore con pallino bianco, come gli switch di macOS
pub fn switch(theme: &Theme, status: toggler::Status) -> toggler::Style {
    let tones = tones(theme);
    let primary = theme.palette().primary;
    let background = match status {
        toggler::Status::Active { is_toggled: true } => primary,
        toggler::Status::Hovered { is_toggled: true } => soft(primary, 0.85),
        toggler::Status::Active { is_toggled: false } => tones.border,
        toggler::Status::Hovered { is_toggled: false } => soft(tones.muted, 0.5),
        toggler::Status::Disabled => soft(tones.border, 0.6),
    };
    toggler::Style {
        background,
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: if status == toggler::Status::Disabled { soft(Color::WHITE, 0.6) } else { Color::WHITE },
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
    }
}

// --- Campi di testo ---

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let tones = tones(theme);
    let base = text_input::default(theme, status);
    let border_color = match status {
        text_input::Status::Focused => theme.palette().primary,
        text_input::Status::Hovered => soft(tones.muted, 0.6),
        _ => tones.border,
    };
    text_input::Style {
        background: tones.card.into(),
        border: Border { color: border_color, width: 1.0, radius: border::radius(RADIUS) },
        placeholder: soft(tones.muted, 0.8),
        ..base
    }
}
