//! The log panel, which can be collapsed and filtered by level

use iced::widget::{Text, button, column, container, row, scrollable, space, text};
use iced::{Alignment, Element, Fill, FillPortion, Font};
use iced_fonts::lucide;
use tracing::Level;

use super::{card, heading, labeled};
use crate::app::{Message, Ret};
use crate::logs::LogFilter;
use crate::style::{self, Tone};

pub fn view(ret: &Ret) -> Element<'_, Message> {
    let logs = &ret.logs;
    let mut title = row![heading(lucide::scroll_text(), "Logs")]
        .spacing(10)
        .align_y(Alignment::Center);
    for (level, tone, singular, plural) in [
        (Level::ERROR, Tone::Danger, "error", "errors"),
        (Level::WARN, Tone::Warning, "warning", "warnings"),
    ] {
        let count = logs.count(level);
        if count > 0 {
            let label = if count == 1 { singular } else { plural };
            title = title.push(
                container(text(format!("{count} {label}")).size(12))
                    .padding([3, 10])
                    .style(style::chip(tone)),
            );
        }
    }
    title = title.push(space().width(Fill));
    if logs.visible {
        for (icon, filter, label) in [
            (lucide::list(), LogFilter::All, "All"),
            (lucide::triangle_alert(), LogFilter::Warnings, "Warnings"),
            (lucide::circle_x(), LogFilter::Errors, "Errors"),
        ] {
            title = title.push(link(
                icon,
                label,
                logs.filter == filter,
                Message::LogFilterChanged(filter),
            ));
        }
        title = title.push(link(lucide::eraser(), "Clear", false, Message::ClearLogs));
    }
    title = title.push(if logs.visible {
        link(lucide::chevron_down(), "Hide", false, Message::ToggleLogs)
    } else {
        link(lucide::chevron_up(), "Show", false, Message::ToggleLogs)
    });

    if !logs.visible {
        return container(title).padding(18).style(style::card).into();
    }

    let lines = column(logs.shown().map(|line| {
        let tone = match line.level {
            Level::ERROR => Some(Tone::Danger),
            Level::WARN => Some(Tone::Warning),
            _ => None,
        };
        let line = text(&line.text).font(Font::MONOSPACE).size(12);
        match tone {
            Some(tone) => line.style(style::tone(tone)).into(),
            None => line.into(),
        }
    }))
    .spacing(3)
    .padding([12, 14]);
    let output: Element<'_, Message> = if logs.shown().next().is_none() {
        let message = match logs.filter {
            LogFilter::All => "Nothing logged yet",
            LogFilter::Warnings => "No warning",
            LogFilter::Errors => "No error",
        };
        container(text(message).size(13).style(style::muted))
            .center(Fill)
            .into()
    } else {
        scrollable(lines)
            .anchor_bottom()
            .width(Fill)
            .height(Fill)
            .into()
    };
    let output = container(output)
        .width(Fill)
        .height(Fill)
        .style(style::inset);

    card(title, output).height(FillPortion(2)).into()
}

fn link<'a>(
    icon: Text<'a>,
    label: &'a str,
    selected: bool,
    message: Message,
) -> Element<'a, Message> {
    button(labeled(icon, label, 12))
        .padding([4, 10])
        .style(style::link(selected))
        .on_press(message)
        .into()
}
