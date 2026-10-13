//! The log panel, which can be collapsed and filtered by level

use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{Text, button, container, keyed_column, row, scrollable, space, text};
use iced::{Alignment, Element, Fill, FillPortion, Font, Pixels};
use iced_fonts::lucide;
use tracing::Level;

use super::smooth::smooth;
use super::tail::tail;
use super::{BOLD, card, heading, labeled};
use crate::app::{Message, Ret};
use crate::logs::{Line, LogFilter, MAX_ROWS};
use crate::style::{self, Tone};

/// Height of a row of text in the panel, the same within a wrapped line and between two lines
const ROW_HEIGHT: f32 = 18.0;
/// Width of the level of a line, the widest one (`ERROR`) included, so that what follows starts
/// at the same position on every line
const LEVEL_WIDTH: f32 = 60.0;

pub fn view(ret: &Ret) -> Element<'_, Message> {
    let logs = &ret.logs;
    let mut title = row![heading(lucide::scroll_text(), "Logs")]
        .spacing(10)
        .align_y(Alignment::Center);
    for (count, tone, singular, plural) in [
        (logs.errors(), Tone::Danger, "error", "errors"),
        (logs.warnings(), Tone::Warning, "warning", "warnings"),
    ] {
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

    // keyed by line, so that the rows already shown keep their laid out text
    let lines = keyed_column(logs.shown().map(|(id, line)| (id, log_line(line)))).width(Fill);
    // at most as many rows as always kept, so that dropping long lines does not change the height
    // (unfiltered, see MAX_ROWS)
    let lines = container(tail(lines, MAX_ROWS as f32 * ROW_HEIGHT)).padding([12, 14]);
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
        smooth(scrollable(lines).anchor_bottom().width(Fill).height(Fill))
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

/// One line: the icon and the level, then the component and the message
fn log_line(line: &Line) -> Element<'_, Message> {
    let (icon, label, tone) = describe_level(line.level);
    let level_text = container(
        row![
            // in a box 4 px shorter, at the top of the row: 2 px higher than centered
            container(
                icon.size(11)
                    .line_height(row_height(ROW_HEIGHT - 4.0))
                    .style(style::tone(tone)),
            )
            .height(ROW_HEIGHT),
            // in a box 2 px shorter, at the top of the row: 1 px higher than centered
            container(
                text(label)
                    .size(11)
                    .font(BOLD)
                    .line_height(row_height(ROW_HEIGHT - 2.0))
                    .wrapping(Wrapping::None)
                    .style(style::tone(tone)),
            )
            .height(ROW_HEIGHT),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .width(LEVEL_WIDTH)
    .height(ROW_HEIGHT)
    .align_y(Alignment::Center);

    // wraps within its own column, aligned under its first row rather than under the level
    let message = text(&line.message)
        .font(Font::MONOSPACE)
        .size(12)
        .line_height(row_height(ROW_HEIGHT))
        .wrapping(Wrapping::WordOrGlyph)
        .width(Fill);
    let message = match line.level {
        Level::ERROR => message.style(style::tone(Tone::Danger)),
        Level::WARN => message.style(style::tone(Tone::Warning)),
        _ => message,
    };

    row![
        level_text,
        text(&line.target)
            .font(Font::MONOSPACE)
            .size(12)
            .line_height(row_height(ROW_HEIGHT))
            .wrapping(Wrapping::None)
            .style(style::muted),
        message,
    ]
    .spacing(10)
    .align_y(Alignment::Start)
    .into()
}

fn row_height(height: f32) -> LineHeight {
    LineHeight::Absolute(Pixels(height))
}

/// The icon, label and color of a log level
fn describe_level(level: Level) -> (Text<'static>, &'static str, Tone) {
    match level {
        Level::ERROR => (lucide::circle_x(), "ERROR", Tone::Danger),
        Level::WARN => (lucide::triangle_alert(), "WARN", Tone::Warning),
        Level::INFO => (lucide::info(), "INFO", Tone::Info),
        Level::DEBUG => (lucide::bug(), "DEBUG", Tone::Primary),
        Level::TRACE => (lucide::footprints(), "TRACE", Tone::Neutral),
    }
}
