#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::{App, Arg};
use cookiebot::{Config, ThePositiveBotBot};
use metrics_exporter_prometheus::PrometheusBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

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

    ThePositiveBotBot::new(
        config.username,
        config.token,
        config.channel,
        accept_invalid_certs,
    )
    .run()
    .await
}
