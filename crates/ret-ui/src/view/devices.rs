//! The devices known to adb and the actions on their client

use iced::widget::{Text, button, column, container, row, scrollable, space, text, tooltip};
use iced::{Alignment, Element, Fill, padding};
use iced_fonts::lucide;

use super::menu::menu;
use super::smooth::smooth;
use super::{BOLD, card, dots, heading, labeled};
use crate::app::{DeviceRow, Message, Ret};
use crate::backend::Action;
use crate::style::{self, Tone};

/// Width of the start and stop button, the same for both labels
const TOGGLE_WIDTH: f32 = 116.0;
/// Height of the start and stop button, and size of the menu button next to it
const BUTTON_SIZE: f32 = 44.0;
/// Width of the menu of a device
const MENU_WIDTH: f32 = 280.0;

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
    let mut title = row![
        heading(lucide::smartphone(), "Devices"),
        count,
        space().width(Fill)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if ret.autostart {
        // changed in the settings, shown here because it decides what plugging a device does
        title = title.push(tooltip(
            container(
                row![lucide::zap().size(12), text("Auto-start").size(12)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding([3, 10])
            .style(style::chip(Tone::Primary)),
            hint("Every device starts its client as soon as adb sees it"),
            tooltip::Position::Bottom,
        ));
    }

    let body: Element<'_, Message> = if ret.devices.is_empty() {
        empty()
    } else {
        smooth(
            scrollable(
                column(ret.devices.iter().map(|row| {
                    let menu_open = ret.menu.as_ref() == Some(&row.device.serial);
                    device(row, menu_open, ret.tick)
                }))
                .spacing(10)
                .padding(padding::right(12)),
            )
            .height(Fill),
        )
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

fn device(row: &DeviceRow, menu_open: bool, tick: usize) -> Element<'_, Message> {
    let available = row.device.is_available();
    let (state_icon, state, tone) = describe_state(&row.device.state);

    let headset = row.info.as_ref().is_some_and(|info| info.headset);
    let kind_icon = if headset {
        lucide::rectangle_goggles()
    } else {
        lucide::smartphone()
    };
    let avatar = container(kind_icon.size(20))
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
        (None, None) if row.running == Some(true) => (
            lucide::circle_play(),
            "Client running".to_owned(),
            Some(Tone::Success),
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
    let name = row.info.as_ref().and_then(|info| info.name.as_deref());
    let mut info = column![
        row![
            text(name.unwrap_or(&row.device.serial)).size(15).font(BOLD),
            chip
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ];
    // the serial stays visible under the name, to tell identical devices apart
    if name.is_some() {
        info = info.push(text(&row.device.serial).size(12).style(style::muted));
    }
    let info = info.push(status).spacing(5).width(Fill);

    let enabled = available && row.busy.is_none();
    let run = |action| enabled.then(|| Message::Run(row.device.serial.clone(), action));
    let (toggle, toggle_action) = if row.running == Some(true) {
        (
            button(centered(labeled(lucide::square(), "Stop", 16))).style(style::danger),
            Action::Stop,
        )
    } else {
        (
            button(centered(labeled(lucide::play(), "Start", 16))).style(style::primary),
            Action::Start,
        )
    };
    let toggle = toggle
        .padding(0)
        .width(TOGGLE_WIDTH)
        .height(BUTTON_SIZE)
        .on_press_maybe(run(toggle_action));

    let more = button(centered(lucide::ellipsis().size(18).into()))
        .padding(0)
        .width(BUTTON_SIZE)
        .height(BUTTON_SIZE)
        .style(style::ghost)
        .on_press(Message::ToggleMenu(row.device.serial.clone()));
    let item = |icon: Text<'static>, label, help, action| {
        button(
            row![
                icon.size(16),
                column![
                    text(label).size(13).font(BOLD),
                    text(help).size(12).style(style::muted)
                ]
                .spacing(2),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([8, 10])
        .width(Fill)
        .style(style::menu_item)
        .on_press_maybe(run(action))
    };
    let menu_items = container(
        column![
            item(
                lucide::refresh_cw(),
                "Reset the tunnel",
                "After the device was plugged back",
                Action::Tunnel,
            ),
            item(
                lucide::download(),
                "Reinstall the client",
                "Uninstall it, then install it again",
                Action::Reinstall,
            ),
        ]
        .spacing(2),
    )
    .padding(6)
    .width(Fill)
    .style(style::card);
    let more = menu(more, menu_items, menu_open, MENU_WIDTH, Message::CloseMenu);

    let actions = row![toggle, more].spacing(8).align_y(Alignment::Center);

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

/// `content` in the middle of a fixed-size button
fn centered(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content).center(Fill).into()
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
