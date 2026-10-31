//! Layout of the window

mod devices;
mod logs;
mod menu;
mod settings;
mod smooth;
mod tail;

use iced::widget::{Text, button, column, container, progress_bar, row, space, text};
use iced::{Alignment, Element, Fill, FillPortion, Font, font, padding};
use iced_fonts::lucide;
use ret_adb::download::Progress;

use crate::app::{Message, Page, Relay, Ret};
use crate::style::{self, Tone};

pub const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

pub fn view(ret: &Ret) -> Element<'_, Message> {
    // the settings scroll up to the bottom of the window, their padding is inside the scrollable
    let bottom = if ret.page == Page::Settings { 0 } else { 24 };
    let mut content = column![header(ret)]
        .spacing(20)
        .padding(padding::all(24).bottom(bottom));
    if let Some(banner) = download_banner(ret) {
        content = content.push(banner);
    }
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
    match ret.page {
        Page::Home => {
            content = content.push(container(devices::view(ret)).height(FillPortion(3)));
            content = content.push(logs::view(ret));
        }
        Page::Settings => content = content.push(settings::view(ret)),
    }
    container(content)
        .width(Fill)
        .height(Fill)
        .style(style::page)
        .into()
}

fn download_banner(ret: &Ret) -> Option<Element<'_, Message>> {
    let (icon, title, tone) = match &ret.download {
        Progress::Idle | Progress::Ready => return None,
        Progress::Downloading { .. } => (
            None,
            format!("Downloading ADB tools{}", dots(ret.tick)),
            Tone::Primary,
        ),
        Progress::Extracting => (
            None,
            format!("Extracting ADB tools{}", dots(ret.tick)),
            Tone::Primary,
        ),
        Progress::Failed(_) => (
            Some(lucide::circle_alert()),
            "ADB tools setup failed".to_owned(),
            Tone::Danger,
        ),
    };
    let mut body = column![text(title).font(BOLD)].spacing(6).width(Fill);
    match &ret.download {
        Progress::Downloading { downloaded, total } => {
            body = body.push(text(ret.download.to_string()).size(13));
            if let Some(total) = total.filter(|&total| total > 0) {
                let fraction = (*downloaded).min(total) as f32 / total as f32;
                body = body.push(progress_bar(0.0..=1.0, fraction).girth(6));
            }
        }
        Progress::Failed(error) => body = body.push(text(error).size(13)),
        _ => {}
    }
    let content = if let Some(icon) = icon {
        row![icon.size(20), body]
    } else {
        row![body]
    };
    Some(
        container(content.spacing(12).align_y(Alignment::Center))
            .padding([12, 16])
            .width(Fill)
            .style(style::banner(tone))
            .into(),
    )
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
        Relay::Stopped { error: None } => (
            lucide::zap_off(),
            "Relay idle until a client starts".to_owned(),
            Tone::Neutral,
        ),
    };
    let status = container(
        row![icon.size(14), text(status).size(13)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([7, 14])
    .style(style::chip(tone));

    let on_settings = ret.page == Page::Settings;
    let settings = button(container(lucide::settings().size(18)).center(Fill))
        .padding(0)
        .width(40)
        .height(40)
        .style(style::link(on_settings))
        .on_press(Message::ShowPage(if on_settings {
            Page::Home
        } else {
            Page::Settings
        }));

    row![logo, title, space().width(Fill), status, settings]
        .spacing(14)
        .align_y(Alignment::Center)
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
