#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::panic, clippy::unwrap_used))]

mod app;
mod browser;
mod ds;
mod native_menu;
mod persistence;
mod renderer;
mod ui;

pub use app::run;
