#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{App, Arg};
use cookiebot::{Config, CookieBot, EgBot};
use git_version::git_version;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::select;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
#[instrument]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Starting with version: git: {}", git_version!());

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

    let cookiebot = CookieBot::new(
        config.username.clone(),
        config.token.clone(),
        config.cookiebot_channel,
        accept_invalid_certs,
    );

    let egbot = EgBot::new(config.username, config.token, config.egbot_channel);

    select! {
        result = cookiebot.run(), if !config.cookiebot_disabled => {
            if let Err(err) = result {
                error!("Error running CookieBot: {}", err);
            }
            warn!("CookieBot finished running");
        }
        result = egbot.run(), if !config.egbot_disabled => {
            if let Err(err) = result {
                error!("Error running EgBot: {}", err);
            }
            warn!("EgBot finished running");
        }
        else => {
            warn!("no bot is configured to run")
        }
    }

    Ok(())
}
