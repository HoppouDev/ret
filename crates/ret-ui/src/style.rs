//! Colors and shapes of the window: Catppuccin Mocha when the system is dark, Latte when light
//!
//! The surfaces follow the Catppuccin style guide: the page is `crust`, cards are `base`, sunken
//! areas are `mantle`, borders are `surface0`, and the accent is `mauve`.

use catppuccin::{FlavorColors, PALETTE};
use iced::theme::{Mode, Palette};
use iced::widget::{button, container, text, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};
use std::sync::LazyLock;

const RADIUS: f32 = 10.0;
const CARD_RADIUS: f32 = 14.0;

static MOCHA: LazyLock<Theme> = LazyLock::new(|| custom("Catppuccin Mocha", &PALETTE.mocha.colors));
static LATTE: LazyLock<Theme> = LazyLock::new(|| custom("Catppuccin Latte", &PALETTE.latte.colors));

/// A theme whose palette, used by the default widget styles, is made of Catppuccin colors
fn custom(name: &'static str, colors: &FlavorColors) -> Theme {
    Theme::custom(
        name,
        Palette {
            background: colors.base.into(),
            text: colors.text.into(),
            primary: colors.mauve.into(),
            success: colors.green.into(),
            warning: colors.yellow.into(),
            danger: colors.red.into(),
        },
    )
}

pub fn theme(mode: Mode) -> Theme {
    match mode {
        Mode::Light => LATTE.clone(),
        Mode::Dark | Mode::None => MOCHA.clone(),
    }
}

/// The Catppuccin flavor of a theme
fn colors(theme: &Theme) -> &'static FlavorColors {
    if theme.extended_palette().is_dark {
        &PALETTE.mocha.colors
    } else {
        &PALETTE.latte.colors
    }
}

fn color(color: catppuccin::Color) -> Color {
    color.into()
}

/// The meaning of a colored element
#[derive(Debug, Clone, Copy)]
pub enum Tone {
    Neutral,
    Primary,
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn color(self, theme: &Theme) -> Color {
        let colors = colors(theme);
        color(match self {
            Tone::Neutral => colors.overlay1,
            Tone::Primary => colors.mauve,
            Tone::Info => colors.blue,
            Tone::Success => colors.green,
            Tone::Warning => colors.yellow,
            Tone::Danger => colors.red,
        })
    }
}

/// The page behind the cards
pub fn page(theme: &Theme) -> container::Style {
    let colors = colors(theme);
    container::Style {
        background: Some(color(colors.crust).into()),
        text_color: Some(color(colors.text)),
        ..container::Style::default()
    }
}

/// A raised surface grouping related content
pub fn card(theme: &Theme) -> container::Style {
    let colors = colors(theme);
    let dark = theme.extended_palette().is_dark;
    container::Style {
        background: Some(color(colors.base).into()),
        border: Border {
            color: color(colors.surface0),
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::BLACK.scale_alpha(if dark { 0.3 } else { 0.06 }),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..container::Style::default()
    }
}

/// A sunken area inside a card, such as a device or the log output
pub fn inset(theme: &Theme) -> container::Style {
    let colors = colors(theme);
    container::Style {
        background: Some(color(colors.mantle).into()),
        border: Border {
            color: color(colors.surface0),
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..container::Style::default()
    }
}

/// A small rounded label tinted with the color of its tone
pub fn chip(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let color = tone.color(theme);
        container::Style {
            background: Some(color.scale_alpha(0.15).into()),
            text_color: Some(color),
            border: Border::default().rounded(999),
            ..container::Style::default()
        }
    }
}

/// A rounded tile holding an icon, tinted with the color of its tone
pub fn tile(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let color = tone.color(theme);
        container::Style {
            background: Some(color.scale_alpha(0.15).into()),
            text_color: Some(color),
            border: Border::default().rounded(RADIUS),
            ..container::Style::default()
        }
    }
}

/// The logo in the header
pub fn logo(theme: &Theme) -> container::Style {
    let colors = colors(theme);
    container::Style {
        background: Some(color(colors.mauve).into()),
        text_color: Some(color(colors.base)),
        border: Border::default().rounded(RADIUS),
        ..container::Style::default()
    }
}

/// A full-width message tinted with the color of its tone
pub fn banner(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let color = tone.color(theme);
        container::Style {
            background: Some(color.scale_alpha(0.12).into()),
            text_color: Some(color),
            border: Border {
                color: color.scale_alpha(0.4),
                width: 1.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        }
    }
}

/// Filled button for the main action
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let colors = colors(theme);
    filled(colors.mauve, colors.base, status)
}

/// Filled button for an action that stops something
pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    let colors = colors(theme);
    filled(colors.red, colors.base, status)
}

fn filled(
    background: catppuccin::Color,
    text: catppuccin::Color,
    status: button::Status,
) -> button::Style {
    let background = color(background);
    let background = match status {
        button::Status::Hovered => background.scale_alpha(0.85),
        button::Status::Pressed => background.scale_alpha(0.7),
        button::Status::Disabled => background.scale_alpha(0.35),
        button::Status::Active => background,
    };
    let alpha = if status == button::Status::Disabled {
        0.6
    } else {
        1.0
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: color(text).scale_alpha(alpha),
        border: Border::default().rounded(RADIUS),
        ..button::Style::default()
    }
}

/// Outlined button for secondary actions
pub fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let colors = colors(theme);
    let background = match status {
        button::Status::Hovered => Some(color(colors.surface0).into()),
        button::Status::Pressed => Some(color(colors.surface1).into()),
        button::Status::Active | button::Status::Disabled => None,
    };
    let alpha = if status == button::Status::Disabled {
        0.4
    } else {
        1.0
    };
    button::Style {
        background,
        text_color: color(colors.text).scale_alpha(alpha),
        border: Border {
            color: color(colors.surface1).scale_alpha(alpha),
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..button::Style::default()
    }
}

/// Borderless button filling a row of a menu, highlighted under the cursor
pub fn menu_item(theme: &Theme, status: button::Status) -> button::Style {
    let colors = colors(theme);
    let background = match status {
        button::Status::Hovered => Some(color(colors.surface0).into()),
        button::Status::Pressed => Some(color(colors.surface1).into()),
        button::Status::Active | button::Status::Disabled => None,
    };
    let alpha = if status == button::Status::Disabled {
        0.4
    } else {
        1.0
    };
    button::Style {
        background,
        text_color: color(colors.text).scale_alpha(alpha),
        border: Border::default().rounded(RADIUS - 2.0),
        ..button::Style::default()
    }
}

/// Text-only button, highlighted when `selected`
pub fn link(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let colors = colors(theme);
        let text_color = if selected || status == button::Status::Hovered {
            colors.text
        } else {
            colors.subtext0
        };
        button::Style {
            background: selected.then(|| color(colors.surface0).into()),
            text_color: color(text_color),
            border: Border::default().rounded(RADIUS - 2.0),
            ..button::Style::default()
        }
    }
}

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let colors = colors(theme);
    let border_color = match status {
        text_input::Status::Focused { .. } => colors.mauve,
        text_input::Status::Hovered => colors.surface2,
        text_input::Status::Active | text_input::Status::Disabled => colors.surface0,
    };
    let disabled = status == text_input::Status::Disabled;
    text_input::Style {
        background: color(colors.mantle).into(),
        border: Border {
            color: color(border_color),
            width: 1.0,
            radius: RADIUS.into(),
        },
        icon: color(colors.overlay1),
        placeholder: color(colors.overlay0),
        value: color(if disabled {
            colors.overlay1
        } else {
            colors.text
        }),
        selection: color(colors.mauve).scale_alpha(0.35),
    }
}

/// Less prominent text, for hints and captions
pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(color(colors(theme).subtext0)),
    }
}

pub fn tone(tone: Tone) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(tone.color(theme)),
    }
}
