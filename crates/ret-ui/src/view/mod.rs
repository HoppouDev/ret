//! Layout of the window

mod devices;
mod logs;

use iced::widget::{Text, button, column, container, row, space, text, text_input};
use iced::{Alignment, Element, Fill, FillPortion, Font, font};
use iced_fonts::lucide;

use crate::app::{Message, Relay, Ret};
use crate::style::{self, Tone};

pub const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

const SIDEBAR_WIDTH: u32 = 300;

pub fn view(ret: &Ret) -> Element<'_, Message> {
    let mut content = column![header(ret)].spacing(20).padding(24);
    if let Relay::Stopped { error: Some(error) } = &ret.relay {
        content = content.push(
            container(
                row![
                    lucide::circle_alert().size(20),
                    column![
                        text("The relay server stopped").font(BOLD),
                        text(error).size(13),
                    ]
                    .spacing(4),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Fill)
            .style(style::banner(Tone::Danger)),
        );
    }
    let body = row![settings(ret), devices::view(ret)].spacing(20);
    content = content.push(body.height(FillPortion(3)));
    content = content.push(logs::view(ret));
    container(content)
        .width(Fill)
        .height(Fill)
        .style(style::page)
        .into()
}

/// The current animation frame of a work in progress
pub fn dots(tick: usize) -> &'static str {
    ["", ".", "..", "..."][tick % 4]
}

/// An icon followed by a label, the content of most buttons
pub fn labeled<'a>(icon: Text<'a>, label: &'a str, size: u32) -> Element<'a, Message> {
    row![icon.size(size), text(label).size(size)]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn header(ret: &Ret) -> Element<'_, Message> {
    let logo = container(lucide::cable().size(22))
        .center(42)
        .style(style::logo);
    let title = column![
        text("ret").size(22).font(BOLD),
        text("Reverse tethering for Android over adb")
            .size(13)
            .style(style::muted),
    ]
    .spacing(2);

    let (icon, status, tone) = match &ret.relay {
        Relay::Running { port, .. } => (
            lucide::zap(),
            format!("Relay running on port {port}"),
            Tone::Success,
        ),
        Relay::Stopping => (
            lucide::loader_circle(),
            format!("Stopping the relay{}", dots(ret.tick)),
            Tone::Warning,
        ),
        Relay::Stopped { error: Some(_) } => (
            lucide::circle_alert(),
            "Relay stopped".to_owned(),
            Tone::Danger,
        ),
        Relay::Stopped { error: None } => {
            (lucide::zap_off(), "Relay stopped".to_owned(), Tone::Neutral)
        }
    };
    let status = container(
        row![icon.size(14), text(status).size(13)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([7, 14])
    .style(style::chip(tone));

    let toggle = match &ret.relay {
        Relay::Running { .. } => button(labeled(lucide::square(), "Stop relay", 14))
            .style(style::danger)
            .on_press(Message::ToggleRelay),
        Relay::Stopping => button(labeled(lucide::square(), "Stop relay", 14)).style(style::danger),
        Relay::Stopped { .. } => button(labeled(lucide::play(), "Start relay", 14))
            .style(style::primary)
            .on_press_maybe(ret.port().map(|_| Message::ToggleRelay)),
    };

    row![
        logo,
        title,
        space().width(Fill),
        status,
        toggle.padding([10, 20])
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

fn settings(ret: &Ret) -> Element<'_, Message> {
    let editable = matches!(ret.relay, Relay::Stopped { .. });
    let port_caption = if !editable {
        text("Stop the relay to change its port").style(style::muted)
    } else if ret.port().is_some() {
        text("Port of the relay server on this computer").style(style::muted)
    } else {
        text("Enter a port between 1 and 65535").style(style::tone(Tone::Danger))
    };

    let connection = card(
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
                text("Comma-separated, used by the devices")
                    .size(12)
                    .style(style::muted),
            ),
            field(
                lucide::route(),
                "Routes",
                input("0.0.0.0/0", &ret.routes).on_input(Message::RoutesChanged),
                text("Comma-separated, the traffic to reverse tether")
                    .size(12)
                    .style(style::muted),
            ),
        ]
        .spacing(16),
    );

    container(connection.height(Fill))
        .width(SIDEBAR_WIDTH)
        .into()
}

fn input<'a>(placeholder: &'a str, value: &'a str) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .padding([9, 12])
        .size(14)
        .style(style::input)
}

fn field<'a>(
    icon: Text<'a>,
    label: &'a str,
    input: impl Into<Element<'a, Message>>,
    caption: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        row![
            icon.size(13).style(style::muted),
            text(label).size(13).font(BOLD)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        input.into(),
        caption.into()
    ]
    .spacing(6)
    .into()
}

/// A titled surface grouping related content
pub fn card<'a>(
    title: impl Into<Element<'a, Message>>,
    body: impl Into<Element<'a, Message>>,
) -> container::Container<'a, Message> {
    container(column![title.into(), body.into()].spacing(16))
        .padding(18)
        .width(Fill)
        .style(style::card)
}

/// The title of a card, after its icon
pub fn heading<'a>(icon: Text<'a>, title: &'a str) -> iced::widget::Row<'a, Message> {
    row![
        icon.size(16).style(style::tone(Tone::Primary)),
        text(title).size(15).font(BOLD)
    ]
    .spacing(8)
    .align_y(Alignment::Center)
}
