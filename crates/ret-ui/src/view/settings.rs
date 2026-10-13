//! The settings page, grouped by category

use iced::widget::{Text, button, column, container, row, scrollable, text, text_input, toggler};
use iced::{Alignment, Element, Fill, Font, padding};
use iced_fonts::lucide;

use super::smooth::smooth;
use super::{BOLD, card, heading, labeled};
use crate::app::{Message, Page, Relay, Ret};
use crate::config::ThemeChoice;
use crate::style::{self, Tone};

/// Width of the settings, which read badly across a wide window
const MAX_WIDTH: u32 = 720;

pub fn view(ret: &Ret) -> Element<'_, Message> {
    let title = row![
        button(labeled(lucide::chevron_left(), "Back", 13))
            .padding([6, 10])
            .style(style::link(false))
            .on_press(Message::ShowPage(Page::Home)),
        heading(lucide::settings(), "Settings"),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let categories = column![title, connection(ret), devices(ret), appearance(ret)]
        .spacing(20)
        .max_width(MAX_WIDTH);
    // the bottom padding of the page, inside so that scrolled content is not cut above it
    let content = container(categories)
        .center_x(Fill)
        .padding(padding::horizontal(12).bottom(24));
    smooth(scrollable(content).height(Fill))
}

fn connection(ret: &Ret) -> Element<'_, Message> {
    let editable = matches!(ret.relay, Relay::Stopped { .. });
    let port_caption = if !editable {
        text("Stop the clients to change the port, the relay uses it").style(style::muted)
    } else if ret.port().is_some() {
        text("Port of the relay server on this computer").style(style::muted)
    } else {
        text("Enter a port between 1 and 65535").style(style::tone(Tone::Danger))
    };

    card(
        heading(lucide::sliders_horizontal(), "Connection"),
        column![
            field(
                lucide::plug(),
                "Port",
                input("31416", &ret.port)
                    .font(Font::MONOSPACE)
                    .on_input_maybe(
                        editable.then_some(Message::PortChanged as fn(String) -> Message)
                    ),
                port_caption.size(12),
            ),
            field(
                lucide::globe(),
                "DNS servers",
                input("8.8.8.8", &ret.dns_servers).on_input(Message::DnsServersChanged),
                caption("Comma-separated, used by the devices"),
            ),
            field(
                lucide::route(),
                "Routes",
                input("0.0.0.0/0", &ret.routes).on_input(Message::RoutesChanged),
                caption("Comma-separated, the traffic to reverse tether"),
            ),
        ]
        .spacing(16),
    )
    .into()
}

fn devices(ret: &Ret) -> Element<'_, Message> {
    card(
        heading(lucide::smartphone(), "Devices"),
        row![
            column![
                label(lucide::zap(), "Auto-start"),
                caption("Start the client on every device as soon as adb sees it"),
            ]
            .spacing(6)
            .width(Fill),
            toggler(ret.autostart).on_toggle(Message::AutostartToggled),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .into()
}

fn appearance(ret: &Ret) -> Element<'_, Message> {
    let choice = |icon, name, choice| {
        button(labeled(icon, name, 13))
            .padding([7, 12])
            .style(style::link(ret.theme_choice == choice))
            .on_press(Message::ThemeChosen(choice))
    };
    card(
        heading(lucide::palette(), "Appearance"),
        field(
            lucide::sun_moon(),
            "Theme",
            row![
                choice(lucide::monitor(), "System", ThemeChoice::System),
                choice(lucide::sun(), "Light", ThemeChoice::Light),
                choice(lucide::moon(), "Dark", ThemeChoice::Dark),
            ]
            .spacing(6),
            caption("System follows the light or dark mode of the computer"),
        ),
    )
    .into()
}

fn input<'a>(placeholder: &'a str, value: &'a str) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .padding([9, 12])
        .size(14)
        .style(style::input)
}

fn label<'a>(icon: Text<'a>, name: &'a str) -> Element<'a, Message> {
    row![
        icon.size(13).style(style::muted),
        text(name).size(13).font(BOLD)
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

fn caption(content: &str) -> Text<'_> {
    text(content).size(12).style(style::muted)
}

fn field<'a>(
    icon: Text<'a>,
    name: &'a str,
    input: impl Into<Element<'a, Message>>,
    caption: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![label(icon, name), input.into(), caption.into()]
        .spacing(6)
        .into()
}
