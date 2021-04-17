use std::{borrow::Cow, time::Duration};

use anyhow::{Context, Result};
use metrics::{gauge, register_gauge, Unit};
use regex::Regex;
use secrecy::ExposeSecret;
use serde::Deserialize;
use tokio::{sync::mpsc::UnboundedReceiver, time::sleep};
use tracing::{debug, info, instrument, warn};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, TCPTransport,
    TwitchIRCClient,
};

use crate::{
    bot::{self, Bot},
    SecretToken, Timestamp,
};

use super::{
    claimcookie::ClaimCookieResponse,
    patterns::{BUY_CDR_BAD, BUY_CDR_GOOD, GENERIC_ANSWER, PRESTIGE_BAD, PRESTIGE_GOOD},
    rank::Rank,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error: {0}")]
    AnyhowError(#[from] anyhow::Error),

    #[error("Could not check chatters: {0}")]
    CheckChattersError(#[source] bot::Error),
}

static COOLDOWN_API: &str = "https://api.roaringiron.com/cooldown";
static METRIC_TOTAL_COOKIES: &str = "cookiebot.cookies.total";
static METRIC_PRESTIGE: &str = "cookiebot.prestige";
static POSITIVE_BOT_USER_ID: &str = "425363834";

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
pub struct CookieBot {
    username: String,
    token: SecretToken,
    channel: String,
    accept_invalid_certs: bool,
}

impl CookieBot {
    pub fn new(
        username: String,
        token: SecretToken,
        channel: String,
        accept_invalid_certs: bool,
    ) -> Self {
        register_gauge!(METRIC_TOTAL_COOKIES, Unit::Count, "total number of cookies");
        register_gauge!(METRIC_PRESTIGE, Unit::Count, "current prestige level");

        Self {
            username,
            token,
            channel,
            accept_invalid_certs,
        }
    }

    #[instrument]
    pub async fn run(&self) -> Result<()> {
        info!("Running CookieBot");

        loop {
            // update metrics
            let response = self.get_user().await?;
            gauge!(METRIC_TOTAL_COOKIES, response.cookies as f64);
            gauge!(METRIC_PRESTIGE, response.prestige as f64);

            self.wait_for_cooldown().await?;

            if !self
                .check_chatters("thepositivebot")
                .await
                .map_err(|err| Error::CheckChattersError(err))?
            {
                warn!(
                    "ThePositiveBot is not in #{}. Suspending bot for 30 minutes",
                    self.channel
                );
                sleep(Duration::from_secs(60 * 30)).await;
                continue;
            }

            let config = ClientConfig::new_simple(StaticLoginCredentials::new(
                self.username.clone(),
                Some(self.token.expose_secret().to_string()),
            ));
            let (mut incoming_messages, client) =
                TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

            client.join(self.channel.clone());

            match self.claim_cookies(&client, &mut incoming_messages).await? {
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
                        if self.buy_cdr(&client, &mut incoming_messages).await? {
                            info!("Cooldown was reset");
                            continue;
                        }
                    }

                    if total >= 5000 {
                        if !self.prestige(&client, &mut incoming_messages).await? {
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

    #[instrument(skip(self))]
    async fn wait_for_cooldown(&self) -> Result<()> {
        info!("Checking cookie cooldown");

        if let Some(duration) = self.get_cookie_cd().await? {
            info!("Cooldown active");

            info!("Waiting for {}", duration.as_readable());
            sleep(duration).await;
        } else {
            info!("Cooldown not active")
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_cookie_cd(&self) -> Result<Option<Duration>> {
        let client = self.get_client()?;

        let response: CooldownResponse = client
            .get(&format!("{}/{}", COOLDOWN_API, self.username))
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

    #[instrument(skip(self))]
    async fn get_user(&self) -> Result<UserResponse<'_>> {
        let client = self.get_client()?;
        let response: UserResponse = client
            .get(&format!(
                "https://api.roaringiron.com/user/{}",
                self.username
            ))
            .send()
            .await?
            .json()
            .await?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        Ok(response)
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn claim_cookies(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<ClaimCookieResponse> {
        info!("Claiming cookies");

        self.communicate(client, incoming_messages, "!cookie")
            .await?
            .parse()
            .context("Could not parse response of cookie command")
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn prestige(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<bool> {
        Ok(self
            .request(
                client,
                incoming_messages,
                "!prestige",
                PRESTIGE_GOOD.clone(),
                PRESTIGE_BAD.clone(),
            )
            .await?)
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn buy_cdr(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<bool> {
        Ok(self
            .request(
                client,
                incoming_messages,
                "!cdr",
                BUY_CDR_GOOD.clone(),
                BUY_CDR_BAD.clone(),
            )
            .await?)
    }
}

impl Bot for CookieBot {
    fn accepts_invalid_certs(&self) -> bool {
        self.accept_invalid_certs
    }

    fn get_channel(&self) -> &str {
        &self.channel
    }

    fn get_bot_id(&self) -> &str {
        POSITIVE_BOT_USER_ID
    }

    fn get_username(&self) -> &str {
        &self.username
    }

    fn get_generic_answer(&self) -> &Regex {
        &*GENERIC_ANSWER
    }
}
