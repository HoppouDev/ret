//! Control of the ret Android app and of the connected devices through adb

mod adb;
pub mod client;
pub mod track;

pub use adb::{Adb, Error};
