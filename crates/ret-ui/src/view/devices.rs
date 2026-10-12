//! The devices known to adb and the actions on their client

use iced::widget::{
    Text, button, column, container, row, scrollable, space, text, toggler, tooltip,
};
use iced::{Alignment, Element, Fill, padding};
use iced_fonts::lucide;

use super::{BOLD, card, dots, heading, labeled};
use crate::app::{DeviceRow, Message, Ret};
use crate::backend::Action;
use crate::style::{self, Tone};

pub fn view(ret: &Ret) -> Element<'_, Message> {
    let available = ret
        .devices
        .iter()
        .filter(|row| row.device.is_available())
        .count();
    let count = container(text(format!("{available} ready")).size(12))
        .padding([3, 10])
        .style(style::chip(if available > 0 {
            Tone::Success
        } else {
            Tone::Neutral
        }));
    let autostart = tooltip(
        row![
            lucide::zap().size(14).style(style::muted),
            toggler(ret.autostart)
                .label("Auto-start")
                .text_size(13)
                .on_toggle(Message::AutostartToggled),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        hint("Start the client on every device as soon as adb sees it"),
        tooltip::Position::Bottom,
    );
    let title = row![
        heading(lucide::smartphone(), "Devices"),
        count,
        space().width(Fill),
        autostart
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = if ret.devices.is_empty() {
        empty()
    } else {
        scrollable(
            column(ret.devices.iter().map(|row| device(row, ret.tick)))
                .spacing(10)
                .padding(padding::right(12)),
        )
        .height(Fill)
        .into()
    };
    card(title, body).height(Fill).into()
}

fn empty<'a>() -> Element<'a, Message> {
    container(
        column![
            container(lucide::usb().size(28))
                .center(64)
                .style(style::tile(Tone::Neutral)),
            text("No device connected").size(16).font(BOLD),
            text("Plug in a device with USB debugging enabled, it shows up here once adb sees it")
                .size(13)
                .style(style::muted)
                .center(),
        ]
        .spacing(10)
        .max_width(360)
        .align_x(Alignment::Center),
    )
    .center(Fill)
    .into()
}

fn device(row: &DeviceRow, tick: usize) -> Element<'_, Message> {
    let available = row.device.is_available();
    let (state_icon, state, tone) = describe_state(&row.device.state);

    let avatar = container(lucide::smartphone().size(20))
        .center(42)
        .style(style::tile(if available { Tone::Primary } else { tone }));

    let (status_icon, status, status_tone) = match (&row.busy, &row.outcome) {
        (Some(action), _) => (
            lucide::loader_circle(),
            format!("{}{}", busy_message(*action), dots(tick)),
            Some(Tone::Primary),
        ),
        (None, Some(Ok(message))) => (lucide::circle_check(), message.clone(), Some(Tone::Success)),
        (None, Some(Err(error))) => (lucide::circle_x(), error.clone(), Some(Tone::Danger)),
        (None, None) if !available => (
            lucide::circle_alert(),
            state_hint(&row.device.state).to_owned(),
            None,
        ),
        (None, None) => (lucide::circle_dashed(), "Ready to start".to_owned(), None),
    };
    let status = row![status_icon.size(12), text(status).size(12)]
        .spacing(6)
        .align_y(Alignment::Center);
    let status: Element<'_, Message> = match status_tone {
        Some(tone) => container(status).style(move |theme| iced::widget::container::Style {
            text_color: Some(tone.color(theme)),
            ..Default::default()
        }),
        None => container(status).style(|theme| iced::widget::container::Style {
            text_color: style::muted(theme).color,
            ..Default::default()
        }),
    }
    .into();

    let chip = container(
        row![state_icon.size(11), text(state).size(11).font(BOLD)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding([2, 8])
    .style(style::chip(tone));
    let info = column![
        row![text(&row.device.serial).size(15).font(BOLD), chip]
            .spacing(8)
            .align_y(Alignment::Center),
        status,
    ]
    .spacing(5)
    .width(Fill);

    let enabled = available && row.busy.is_none();
    let action = |icon, label, action, style: fn(&_, _) -> _, help| {
        let button = button(labeled(icon, label, 13))
            .padding([7, 12])
            .style(style)
            .on_press_maybe(enabled.then(|| Message::Run(row.device.serial.clone(), action)));
        tooltip(button, hint(help), tooltip::Position::Top)
    };
    let actions = row![
        action(
            lucide::play(),
            "Start",
            Action::Start,
            style::primary,
            "Install the client if needed, then start it"
        ),
        action(
            lucide::square(),
            "Stop",
            Action::Stop,
            style::ghost,
            "Stop the client"
        ),
        action(
            lucide::refresh_cw(),
            "Tunnel",
            Action::Tunnel,
            style::ghost,
            "Reset the adb reverse tunnel, after the device was plugged back"
        ),
        action(
            lucide::download(),
            "Reinstall",
            Action::Reinstall,
            style::ghost,
            "Uninstall then install the client"
        ),
    ]
    .spacing(6);

    container(
        row![avatar, info, actions]
            .spacing(14)
            .align_y(Alignment::Center),
    )
    .padding(14)
    .width(Fill)
    .style(style::inset)
    .into()
}

fn hint(message: &str) -> Element<'_, Message> {
    container(text(message).size(12))
        .padding([6, 10])
        .style(style::card)
        .into()
}

/// An icon, a readable label and a tone for an adb device state
fn describe_state(state: &str) -> (Text<'static>, String, Tone) {
    match state {
        "device" => (lucide::circle_check(), "Ready".to_owned(), Tone::Success),
        "unauthorized" => (lucide::lock(), "Unauthorized".to_owned(), Tone::Warning),
        "offline" => (lucide::unplug(), "Offline".to_owned(), Tone::Danger),
        other => {
            let mut label = other.to_owned();
            if let Some(first) = label.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            (lucide::circle_dashed(), label, Tone::Neutral)
        }
    }
}

fn state_hint(state: &str) -> &'static str {
    match state {
        "unauthorized" => "Allow USB debugging on the device",
        "offline" => "Reconnect the device or restart adb",
        _ => "Waiting for the device",
    }
}

fn busy_message(action: Action) -> &'static str {
    match action {
        Action::Start => "Starting the client",
        Action::Stop => "Stopping the client",
        Action::Reinstall => "Reinstalling the client",
        Action::Tunnel => "Resetting the tunnel",
    }
}
