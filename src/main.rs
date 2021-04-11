#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{App, Arg};
use cookiebot::{secrettoken::Token, Config, CookieBot, EgBot, SecretToken};
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::join;
use tracing::instrument;

#[tokio::main]
#[instrument]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let secret_token = SecretToken::new(Token::new("test"));
    dbg!(ron::to_string(&secret_token).unwrap());

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

    let (cookiebot_result, egbot_result) = join!(cookiebot.run(), egbot.run());
    cookiebot_result?;
    egbot_result?;

    Ok(())
}
