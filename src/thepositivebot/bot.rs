use super::constants::*;
use crate::{twitch::connect, Timestamp, Toggle};
use anyhow::{anyhow, bail, Context, Result};
use log::{debug, info, trace, warn};
use regex::Captures;
use serde::Deserialize;
use smol::{future::FutureExt, Timer};
use std::time::Duration;
use twitchchat::{commands, messages, AsyncRunner, Status, UserConfig};

pub fn total_from_captures(captures: Captures) -> Result<u64> {
    let total = captures
        .name("total")
        .map(|m| m.as_str().parse::<u64>())
        .context("could not get total")?
        .context("could not parse total")?;

    Ok(total)
}

// {
//     "can_claim": false,
//     "interval_formatted": "2 hours",
//     "interval_unformatted": 7200,
//     "seconds_left": 7037.756,
//     "time_left_formatted": "1 hr, 57 mins, and 18 secs",
//     "time_left_unformatted": "01:57:17"
// }
#[derive(Debug, Copy, Clone, Deserialize)]
struct CooldownResponse {
    can_claim: bool,
    seconds_left: f32,
}

#[derive(Debug)]
pub struct Bot {
    user_config: UserConfig,
    channel: String,
    runner: AsyncRunner,
    send_byte: bool,
}

impl Bot {
    pub async fn new(user_config: &UserConfig, channel: &str) -> Result<Self> {
        let runner = connect(&user_config, channel).await?;

        Ok(Self {
            user_config: user_config.clone(),
            channel: channel.to_owned(),
            runner,
            send_byte: false,
        })
    }

    pub async fn main_loop(&mut self) -> Result<()> {
        self.wait_for_cooldown().await?;

        loop {
            info!("Claiming cookies");

            if let Some((cookie, amount, total)) = self.claim_cookies().await? {
                if amount == 0 {
                    info!("No cookies found");
                } else {
                    info!("Got {} {}s", amount, cookie);
                }

                if amount > 7 {
                    info!("Trying to buy cooldown reduction for 7 cookies");
                    if self.buy_cdr().await? {
                        info!("Cooldown was reset");
                        continue;
                    }
                }

                if total >= 5000 {
                    if self.upgrade_prestige().await? {
                        warn!(
                            "Could not upgrade prestige but cookie count is over 5000 ({})",
                            total
                        );
                    }
                }

                info!("Waiting for cooldown");
            } else {
                info!("Could not claim cookies: Cooldown active");
            }

            self.wait_for_cooldown().await?;
        }
    }

    async fn wait_for_cooldown(&mut self) -> Result<()> {
        info!("Checking cookie cooldown");
        match self.get_cookie_cd()? {
            None => {
                info!("Cooldown not active");
            }
            Some(duration) => {
                info!("Cooldown active. Waiting for {}", duration.as_readable());
                Timer::after(duration).await;
            }
        }

        Ok(())
    }

    fn get_cookie_cd(&mut self) -> Result<Option<Duration>> {
        let client = reqwest::blocking::Client::new();
        let response: CooldownResponse = client
            .get(&format!(
                "https://api.roaringiron.com/cooldown/{}",
                self.user_config.name
            ))
            .header(
                "User-Agent",
                concat!(env!("CARGO_PKG_NAME"), " / ", env!("CARGO_PKG_VERSION")),
            )
            .header("X-Github-Repo", env!("CARGO_PKG_REPOSITORY"))
            .send()?
            .json()?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        if response.can_claim {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs_f32(response.seconds_left)))
        }
    }

    async fn claim_cookies(&mut self) -> Result<Option<(String, i32, u64)>> {
        let msg = self.communicate("!cookie").await?;

        if let Some(captures) = CLAIM_GOOD.captures(&msg) {
            let cookie = captures
                .name("cookie")
                .map(|m| m.as_str())
                .context("could not get cookie name")?
                .to_string();

            let amount = captures
                .name("amount")
                .map(|m| m.as_str())
                .map(|s| {
                    s.trim_start_matches('±').parse::<i32>().map(|n| {
                        if s.starts_with('-') {
                            -n
                        } else {
                            n
                        }
                    })
                })
                .context("could not get amount")?
                .context("could not parse amount")?;

            let total = total_from_captures(captures)?;

            return Ok(Some((cookie, amount, total)));
        }

        if CLAIM_BAD.is_match(&msg) {
            return Ok(None);
        }

        Err(anyhow!("no regex matched"))
    }

    async fn upgrade_prestige(&mut self) -> Result<bool> {
        let msg = self.communicate("!prestige").await?;

        if PRESTIGE_GOOD.is_match(&msg) {
            Ok(true)
        } else if PRESTIGE_BAD.is_match(&msg) {
            Ok(false)
        } else {
            Err(anyhow!("no regex matched"))
        }
    }

    async fn buy_cdr(&mut self) -> Result<bool> {
        let msg = self.communicate("!cdr").await?;

        if BUY_CDR_GOOD.is_match(&msg) {
            Ok(true)
        } else if BUY_CDR_BAD.is_match(&msg) {
            Ok(false)
        } else {
            Err(anyhow!("no regex matched"))
        }
    }

    async fn wait_for_answer(&mut self) -> Result<String> {
        use messages::Commands::*;
        debug!("Waiting for response");

        let bot_name = self.user_config.name.clone();

        loop {
            if let Privmsg(msg) = self.next_message().await? {
                if msg.user_id() != Some(POSITIVE_BOT_USER_ID) {
                    trace!("UserID not matching");
                    continue;
                }

                if let Some(captures) = GENERIC_ANSWER.captures(msg.data()) {
                    let username = captures
                        .name("username")
                        .context("could not get username")?
                        .as_str();

                    if username == bot_name {
                        return Ok(msg.data().to_string());
                    }
                }
            }
        }
    }

    async fn communicate(&mut self, message: &str) -> Result<String> {
        const MAX_RETRIES: u32 = 3;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying communication: Retry {}", retry)
            }

            self.say(message).await?;

            return match self
                .wait_for_answer()
                .or(async {
                    // time out after 5 seconds of no response
                    Timer::after(Duration::from_secs(5)).await;
                    info!("Got no response: time out");

                    Err(anyhow!("Response timed out"))
                })
                .await
            {
                Err(response) if response.to_string() == "Response timed out" => {
                    // exponential back off after time out
                    let sleep = Duration::from_secs(2u64.pow(retry + 2));
                    info!("Sleeping for {}", sleep.as_readable());
                    Timer::after(sleep).await;
                    continue;
                }
                result => result,
            };
        }

        Err(anyhow!(
            "Communication failed after {} attempts",
            MAX_RETRIES
        ))
    }

    async fn say(&mut self, message: &str) -> Result<()> {
        let mut message = String::from(message);
        if self.send_byte {
            message.push('\u{e0000}');
        }
        self.send_byte.toggle();

        self.runner
            .writer()
            .encode(commands::privmsg(&self.channel, &message))
            .await?;

        Ok(())
    }

    async fn next_message(&mut self) -> Result<messages::Commands<'_>> {
        use messages::Commands::*;

        loop {
            match self.runner.next_message().await? {
                // this is the parsed message -- across all channels (and notifications from Twitch)
                Status::Message(msg) => {
                    if let Reconnect(_) = msg {
                        self.reconnect().await?;
                    } else {
                        return Ok(msg);
                    }
                }

                // you signaled a quit
                Status::Quit => {
                    warn!("Got quit from twitchchat");
                    bail!("Quitting");
                }

                // the connection closed normally
                Status::Eof => {
                    warn!("Got a 'normal' eof");
                    self.reconnect().await?;
                }
            }
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        info!("Reconnecting");

        self.runner = connect(&self.user_config, &self.channel).await?;

        Ok(())
    }
}
