//! Window to control ret: the relay server and the client on each device

// a release build on Windows opens no console window next to the application window
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod backend;
mod config;
mod logs;
mod style;
mod view;

use app::Ret;
use iced::{Size, window};

/// the icon of the window, decoded from the `.ico` bundle with every size in it
const ICON: &[u8] = include_bytes!("../../../assets/ret.ico");

fn main() -> iced::Result {
    // writes the last lines to the log file once the window closes
    let _log_file = logs::init();
    let _adb_server = ret_adb::ServerGuard::new();
    iced::application(Ret::boot, Ret::update, view::view)
        .title("Ret")
        .theme(Ret::theme)
        .subscription(Ret::subscription)
        .font(iced_fonts::LUCIDE_FONT_BYTES)
        .window(window::Settings {
            size: Size::new(1080.0, 780.0),
            min_size: Some(Size::new(900.0, 640.0)),
            icon: ret_icon(),
            ..window::Settings::default()
        })
        .centered()
        .run()
}

/// the icon of the window: the largest frame of the `.ico`, scaled to the 32x32 size which
/// Windows asks for, see the notes on `SetIcon`
fn ret_icon() -> Option<window::Icon> {
    let frame = image::load_from_memory(ICON).ok()?.into_rgba8();
    let scaled = image::imageops::resize(&frame, 32, 32, image::imageops::FilterType::Lanczos3);
    window::icon::from_rgba(scaled.into_raw(), 32, 32).ok()
}
