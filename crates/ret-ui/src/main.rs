//! Window to control ret: the relay server and the client on each device

// a release build on Windows opens no console window next to the application window
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod backend;
mod logs;
mod style;
mod view;

use app::Ret;
use iced::{Size, window};

fn main() -> iced::Result {
    logs::init();
    iced::application(Ret::boot, Ret::update, view::view)
        .title("ret")
        .theme(Ret::theme)
        .subscription(Ret::subscription)
        .font(iced_fonts::LUCIDE_FONT_BYTES)
        .window(window::Settings {
            size: Size::new(1080.0, 780.0),
            min_size: Some(Size::new(900.0, 640.0)),
            ..window::Settings::default()
        })
        .centered()
        .run()
}
