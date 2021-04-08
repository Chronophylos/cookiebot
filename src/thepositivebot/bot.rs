use crate::{twitch::connect, Timestamp, Toggle};
use anyhow::{anyhow, bail, Context, Result};
use log::{debug, info, trace, warn};
use metrics::{gauge, register_gauge, Unit};
use regex::Regex;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::{borrow::Cow, fmt, ops::Deref, time::Duration};
use tokio::time::{sleep, timeout};
use tracing::instrument;
use twitchchat::{commands, messages, AsyncRunner, Status, UserConfig};

use super::{
    claimcookie::ClaimCookieResponse,
    constants::{
        BUY_CDR_BAD, BUY_CDR_GOOD, GENERIC_ANSWER, POSITIVE_BOT_USER_ID, PRESTIGE_BAD,
        PRESTIGE_GOOD,
    },
    rank::Rank,
};

const COOLDOWN_API: &str = "https://api.roaringiron.com/cooldown";

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

/*
{
  "id": "25790355",
  "username": "chronophylos",
  "twitchID": "54946241",
  "firstseen": "Sun Aug 02 2020 21:16:28 GMT+0000 (Coordinated Universal Time)",
  "lastseen": "Thu Nov 12 2020 11:08:12 GMT+0000 (Coordinated Universal Time)",
  "cookies": 728,
  "rank": "default",
  "prestige": 1,
  "active": "false",
  "cooldownreset_cooldown": "Thu Nov 12 2020 11:08:02 GMT+0000 (Coordinated Universal Time)",
  "booster_cooldown": "none",
  "tip_cooldown": "none"
}
*/
#[derive(Debug, Clone, Deserialize)]
struct UserResponse<'a> {
    cookies: u32,
    rank: Rank,
    prestige: u32,
    booster_cooldown: Cow<'a, str>,
}

#[derive(Debug)]
pub struct Bot {
    user_config: UserConfig,
    channel: String,
    runner: AsyncRunner,
    send_byte: bool,
    accept_invalid_certs: bool,
}

/// Connect to twitch and retry until it worked
#[instrument]
async fn connect_and_retry(user_config: &UserConfig, channel: &str) -> Result<AsyncRunner> {
    loop {
        match connect(user_config, channel).await {
            Ok(runner) => return Ok(runner),
            Err(twitchchat::RunnerError::UnexpectedEof) => continue,
            Err(err) => return Err(err).context("Could not connect to twitch"),
        }
    }
}

const METRIC_TOTAL_COOKIES: &str = "cookiebot.cookies.total";
const METRIC_PRESTIGE: &str = "cookiebot.prestige";

impl Bot {
    pub async fn new(user_config: UserConfig, channel: String, enable_ssl: bool) -> Result<Self> {
        let runner = connect_and_retry(&user_config, &channel).await?;
        register_gauge!(METRIC_TOTAL_COOKIES, Unit::Count, "total number of cookies");
        register_gauge!(METRIC_PRESTIGE, Unit::Count, "current prestige level");

        Ok(Self {
            user_config,
            channel,
            runner,
            send_byte: false,
            accept_invalid_certs: enable_ssl,
        })
    }

    pub async fn main_loop(&mut self) -> Result<()> {
        loop {
            // update metrics
            let response = self.get_user().await?;
            gauge!(METRIC_TOTAL_COOKIES, response.cookies as f64);
            gauge!(METRIC_PRESTIGE, response.prestige as f64);

            self.wait_for_cooldown().await?;

            match self.claim_cookies().await? {
                ClaimCookieResponse::Success {
                    rank,
                    name,
                    amount,
                    total,
                } => {
                    gauge!(METRIC_TOTAL_COOKIES, total as f64);
                    gauge!(METRIC_PRESTIGE, rank.prestige as f64);

                    if amount == 0 {
                        info!("No cookies found");
                    } else {
                        info!("Got {} {}s", amount, name);
                    }

                    if amount > 7 {
                        info!("Trying to buy cooldown reduction for 7 cookies");
                        if self.buy_cdr().await? {
                            info!("Cooldown was reset");
                            continue;
                        }
                    }

                    if total >= 5000 {
                        if !self.prestige().await? {
                            warn!(
                                "Could not upgrade prestige but cookie count is over 5000 ({})",
                                total
                            );
                        }
                    }

                    info!("Waiting for cooldown");
                }
                ClaimCookieResponse::Cooldown { rank, total } => {
                    gauge!(METRIC_TOTAL_COOKIES, total as f64);
                    gauge!(METRIC_PRESTIGE, rank.prestige as f64);

                    info!("Could not claim cookies: Cooldown active");
                }
            }
        }
    }

    #[instrument]
    async fn wait_for_cooldown(&mut self) -> Result<()> {
        info!("Checking cookie cooldown");

        if let Some(duration) = self.get_cookie_cd().await? {
            info!("Cooldown active");

            debug!("Terminating twitch connection");
            self.runner.quit_handle().notify().await;

            info!("Waiting for {}", duration.as_readable());
            sleep(duration).await;

            debug!("Restoring twitch connection");
            self.reconnect().await?;
        } else {
            info!("Cooldown not active")
        }

        Ok(())
    }

    #[instrument]
    async fn get_cookie_cd(&mut self) -> Result<Option<Duration>> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.accept_invalid_certs)
            .build()
            .context("Could not build client")?;

        let response: CooldownResponse = client
            .get(&format!("{}/{}", COOLDOWN_API, self.user_config.name))
            .header(
                USER_AGENT,
                concat!(env!("CARGO_PKG_NAME"), " / ", env!("CARGO_PKG_VERSION")),
            )
            .header("X-Client-Repository", env!("CARGO_PKG_REPOSITORY"))
            .send()
            .await
            .context("Could not send request to api.roaringiron.com")?
            .json()
            .await
            .context("Could not deserialize json response")?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        if response.can_claim {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs_f32(response.seconds_left)))
        }
    }

    #[instrument]
    async fn get_user(&mut self) -> Result<UserResponse<'_>> {
        let client = reqwest::Client::new();
        let response: UserResponse = client
            .get(&format!(
                "https://api.roaringiron.com/user/{}",
                self.user_config.name
            ))
            .header(
                "User-Agent",
                concat!(env!("CARGO_PKG_NAME"), " / ", env!("CARGO_PKG_VERSION")),
            )
            .header("X-Github-Repo", env!("CARGO_PKG_REPOSITORY"))
            .send()
            .await?
            .json()
            .await?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        Ok(response)
    }

    #[instrument]
    async fn claim_cookies(&mut self) -> Result<ClaimCookieResponse> {
        info!("Claiming cookies");

        self.communicate("!cookie")
            .await?
            .parse()
            .context("Could not parse response of cookie command")
    }

    #[instrument]
    async fn prestige(&mut self) -> Result<bool> {
        self.request("!prestige", &PRESTIGE_GOOD, &PRESTIGE_BAD)
            .await
    }

    #[instrument]
    async fn buy_cdr(&mut self) -> Result<bool> {
        self.request("!cdr", &BUY_CDR_GOOD, &BUY_CDR_BAD).await
    }

    #[instrument]
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

    #[instrument]
    async fn communicate(&mut self, message: &str) -> Result<String> {
        const MAX_RETRIES: u32 = 3;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying communication: Retry {}", retry)
            }

            self.say(message).await?;

            return match timeout(Duration::from_secs(5), self.wait_for_answer()).await {
                Err(_elapsed) => {
                    // exponential back off after time out
                    let duration = Duration::from_secs(2u64.pow(retry + 2));
                    info!("Sleeping for {}", duration.as_readable());
                    sleep(duration).await;
                    continue;
                }
                Ok(result) => result,
            };
        }
        Err(anyhow!(
            "Communication failed after {} attempts",
            MAX_RETRIES
        ))
    }

    #[instrument]
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

    #[instrument]
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
                    warn!("Got unexpected quit from twitchchat");
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

        self.runner = connect_and_retry(&self.user_config, &self.channel).await?;

        Ok(())
    }

    #[instrument]
    async fn request<ReGood, ReBad>(
        &mut self,
        message: &str,
        re_good: &ReGood,
        re_bad: &ReBad,
    ) -> Result<bool>
    where
        ReGood: Deref<Target = Regex> + fmt::Debug,
        ReBad: Deref<Target = Regex> + fmt::Debug,
    {
        let response = self.communicate(message).await?;

        if re_good.is_match(&response) {
            Ok(true)
        } else if re_bad.is_match(&response) {
            Ok(false)
        } else {
            Err(anyhow!("no regex matched"))
        }
    }
}
