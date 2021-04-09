use anyhow::{anyhow, Context, Result};
use metrics::{gauge, register_gauge, Unit};
use regex::Regex;
use reqwest::header::{HeaderMap, FROM, USER_AGENT};
use serde::Deserialize;
use std::{borrow::Cow, fmt, ops::Deref, time::Duration};
use tokio::{
    sync::mpsc::UnboundedReceiver,
    time::{sleep, timeout},
};
use tracing::{debug, info, instrument, trace, warn};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, TCPTransport,
    TwitchIRCClient,
};

use super::{
    claimcookie::ClaimCookieResponse,
    constants::{
        BUY_CDR_BAD, BUY_CDR_GOOD, GENERIC_ANSWER, POSITIVE_BOT_USER_ID, PRESTIGE_BAD,
        PRESTIGE_GOOD,
    },
    error::Error,
    rank::Rank,
};
use crate::{Timestamp, Toggle};

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

pub struct Bot {
    username: String,
    token: String,
    channel: String,
    send_byte: bool,
    accept_invalid_certs: bool,
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bot")
            .field("username", &self.username)
            .field("token", &"[redacted]")
            .field("channel", &self.channel)
            .field("send_byte", &self.send_byte)
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .finish()
    }
}

const METRIC_TOTAL_COOKIES: &str = "cookiebot.cookies.total";
const METRIC_PRESTIGE: &str = "cookiebot.prestige";

impl Bot {
    pub async fn new(
        username: String,
        token: String,
        channel: String,
        enable_ssl: bool,
    ) -> Result<Self> {
        register_gauge!(METRIC_TOTAL_COOKIES, Unit::Count, "total number of cookies");
        register_gauge!(METRIC_PRESTIGE, Unit::Count, "current prestige level");

        Ok(Self {
            username,
            token,
            channel,
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

            let config = ClientConfig::new_simple(StaticLoginCredentials::new(
                self.username.clone(),
                Some(self.token.clone()),
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

    fn get_client(&self) -> Result<reqwest::Client, Error> {
        let mut headers = HeaderMap::new();
        headers.append(
            USER_AGENT,
            concat!(env!("CARGO_PKG_NAME"), " / ", env!("CARGO_PKG_VERSION"))
                .parse()
                .map_err(|err| Error::ParsingHeaderValueError(err))?,
        );
        headers.append(
            "X-Github-Repo",
            env!("CARGO_PKG_REPOSITORY")
                .parse()
                .map_err(|err| Error::ParsingHeaderValueError(err))?,
        );
        // cant scrape that email :)
        headers.append(
            FROM,
            String::from_utf8_lossy(&[
                97, 98, 117, 115, 101, 64, 99, 104, 114, 111, 110, 111, 112, 104, 121, 108, 111,
                115, 46, 99, 111, 109,
            ])
            .parse()
            .map_err(|err| Error::ParsingHeaderValueError(err))?,
        );

        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(self.accept_invalid_certs)
            .default_headers(headers)
            // .http2_prior_knowledge()
            .build()
            .map_err(|err| Error::BuildReqwestClientError(err))
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
        &mut self,
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
        &mut self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<bool> {
        self.request(
            client,
            incoming_messages,
            "!prestige",
            &PRESTIGE_GOOD,
            &PRESTIGE_BAD,
        )
        .await
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn buy_cdr(
        &mut self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<bool> {
        self.request(
            client,
            incoming_messages,
            "!cdr",
            &BUY_CDR_GOOD,
            &BUY_CDR_BAD,
        )
        .await
    }

    #[instrument(skip(self, incoming_messages))]
    async fn wait_for_answer(
        &mut self,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<String, Error> {
        debug!("Waiting for response");

        let bot_name = self.username.clone();

        while let Some(server_message) = incoming_messages.recv().await {
            debug!("received message: {:?}", &server_message);

            match server_message {
                ServerMessage::Privmsg(msg) => {
                    if msg.sender.id != POSITIVE_BOT_USER_ID {
                        trace!("UserID not matching");
                        continue;
                    }

                    if let Some(captures) = GENERIC_ANSWER.captures(&msg.message_text) {
                        let username = captures
                            .name("username")
                            .expect("could not get username")
                            .as_str();

                        if username == bot_name {
                            return Ok(msg.message_text);
                        }
                    }
                }
                ServerMessage::Notice(msg) => {
                    if msg.message_text == "Login authentication failed" {
                        return Err(Error::AuthenticateChatError);
                    }
                }
                _ => {}
            }
        }

        Err(Error::ReceivedNoMessageError)
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn communicate(
        &mut self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
        message: &str,
    ) -> Result<String, Error> {
        const MAX_RETRIES: u32 = 3;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying communication: Retry {}", retry)
            }

            self.say(client, message).await?;

            return match timeout(
                Duration::from_secs(5),
                self.wait_for_answer(incoming_messages),
            )
            .await
            {
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

        Err(Error::FailedCommunicationError(MAX_RETRIES))
    }

    #[instrument(skip(self, client))]
    async fn say(
        &mut self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        message: &str,
    ) -> Result<()> {
        let mut message = String::from(message);
        if self.send_byte {
            message.push('\u{E0000}');
        }
        self.send_byte.toggle();

        client.say(self.channel.clone(), message).await?;

        Ok(())
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn request<ReGood, ReBad>(
        &mut self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
        message: &str,
        re_good: &ReGood,
        re_bad: &ReBad,
    ) -> Result<bool>
    where
        ReGood: Deref<Target = Regex> + fmt::Debug,
        ReBad: Deref<Target = Regex> + fmt::Debug,
    {
        let response = self.communicate(client, incoming_messages, message).await?;

        if re_good.is_match(&response) {
            Ok(true)
        } else if re_bad.is_match(&response) {
            Ok(false)
        } else {
            Err(anyhow!("no regex matched"))
        }
    }
}
