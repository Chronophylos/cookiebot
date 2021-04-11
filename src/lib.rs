#![warn(
    missing_debug_implementations,
    missing_copy_implementations,
    clippy::missing_const_for_fn
)]

mod bot;
mod config;
mod okayegbot;
mod thepositivebot;
mod timestamp;

pub mod secrettoken;

pub use config::Config;
pub use okayegbot::EgBot;
pub use secrettoken::SecretToken;
pub use thepositivebot::CookieBot;
pub use timestamp::Timestamp;
