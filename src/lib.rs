#![warn(
    missing_debug_implementations,
    missing_copy_implementations,
    clippy::missing_const_for_fn
)]

mod bot;
mod config;
mod thepositivebot;
mod timestamp;

pub use config::Config;
pub use thepositivebot::CookieBot;
pub use timestamp::Timestamp;
