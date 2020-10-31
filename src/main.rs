#![forbid(unsafe_code)]

use anyhow::Result;
use cookiebot::Config;
use cookiebot::ThePositiveBotBot;
use lazy_static::lazy_static;
use log::info;
use twitchchat::{twitch::Capability, UserConfig};

lazy_static! {
    static ref CONFIG: Config = Config::from_path("cookiebot.ron").unwrap();
}

/*
macro_rules! gen_capture_fun {
    ($re_left:expr, $re_right:expr, $name:ident, $msg:literal, $left:ty, $right:ty) => {
        async fn $name(&mut self) -> Result<Either<$left, $right>> {
            let answer = self.communicate($msg).await?;


            if let Some(captures) = $re_left.captures(&answer) {
                for name in $re_left.capture_names() {

                }
            }


            Err(anyhow!("no regex matched"))
        }
    };
}
*/

#[cfg(not(debug_assertions))]
fn update() -> Result<()> {
    use self_update::{backends::github::Update, cargo_crate_version, Status::*};

    let status = Update::configure()
        .repo_owner("Chronophylos")
        .repo_name("cookiebot")
        .bin_name(env!("CARGO_BIN_NAME"))
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;

    match status {
        UpToDate(_) => info!("Version is up to date"),
        Updated(version) => info!("Updated to {}", version),
    }

    Ok(())
}

#[cfg(debug_assertions)]
fn update() -> Result<()> {
    info!("Running in dev mode. Skipping version check.");

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    update()?;

    let user_config = UserConfig::builder()
        .name(&CONFIG.username)
        .token(&CONFIG.token)
        .capabilities(&[Capability::Tags])
        .build()?;

    smol::block_on(async {
        ThePositiveBotBot::new(&user_config, &CONFIG.channel)
            .await?
            .main_loop()
            .await
    })
}
