#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{App, Arg};
use cookiebot::{Config, ThePositiveBotBot};
use log::{error, info};
use metrics_exporter_prometheus::PrometheusBuilder;
use twitchchat::{twitch::Capability, UserConfig};

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
        Updated(version) => {
            info!("Updated to {}", version);
            use std::process::{exit, Command};

            let code = Command::new(std::env::args().next().expect("executable path"))
                .spawn()?
                .wait()?;

            if code.success() == false {
                exit(code.code().unwrap_or(1));
            } else {
                exit(0);
            }
        }
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

    PrometheusBuilder::new()
        .install()
        .context("could not install Prometheus recorder")?;

    let matches = App::new("cookiebot")
        .arg(
            Arg::with_name("config")
                .long("config")
                .value_name("CONFIG")
                .help("Set a custom config file")
                .default_value("cookiebot.ron")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("accept-invalid-certs")
                .long("accept-invalid-certs")
                .help("(Dangerous) Accept invalid certificates"),
        )
        .get_matches();

    let config_path = matches
        .value_of("config")
        .expect("user set or default config path");
    let config = Config::from_path(config_path)?;

    let accept_invalid_certs = matches.is_present("accept-invalid-certs");

    if let Err(err) = update() {
        error!("Could not update binary: {}", err);
    }

    let user_config = UserConfig::builder()
        .name(config.username)
        .token(config.token)
        .capabilities(&[Capability::Tags])
        .build()?;

    smol::block_on({
        let channel = config.channel.clone();

        async move {
            ThePositiveBotBot::new(user_config, channel, accept_invalid_certs)
                .await?
                .main_loop()
                .await
        }
    })
}
