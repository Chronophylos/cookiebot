use std::time::Duration;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use secrecy::ExposeSecret;
use serde::Deserialize;
use tokio::{sync::mpsc::UnboundedReceiver, time::sleep};
use tracing::{debug, error, info, instrument, trace, warn};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, TCPTransport,
    TwitchIRCClient,
};

use crate::{
    bot::{self, Bot},
    SecretToken, Timestamp,
};

use super::{
    parser::{ClaimEgs, ClaimEgsParserError},
    patterns::GENERIC_ANSWER,
};

static OKAYEG_BOT_USER_ID: &str = "75501168";

lazy_static! {
    static ref CLAIM_EGS_COOLDOWN: chrono::Duration = chrono::Duration::hours(1);
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not communicate with target bot: {0}")]
    Communication(#[source] bot::Error),

    #[error("Could not parse claim egs message: {0}")]
    ParseClaimEgs(#[from] ClaimEgsParserError),

    #[error("Could not get client: {0}")]
    GetClient(#[source] bot::Error),

    #[error("Could not send request: {0}")]
    SendRequest(#[source] reqwest::Error),

    #[error("Could not deserialize response: {0}")]
    DeserializeResponse(#[source] reqwest::Error),

    #[error("Could not check chatters: {0}")]
    CheckChatters(#[source] bot::Error),

    #[error("Request returned bad status code: {0}")]
    BadStatusCode(#[source] reqwest::Error),
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    userid: u64,
    username: String,
    egs: i32,
    cooldown: DateTime<Utc>,
}

#[derive(Debug)]
pub struct EgBot {
    username: String,
    token: SecretToken,
    channel: String,
}

impl EgBot {
    pub fn new(username: String, token: SecretToken, channel: String) -> Self {
        Self {
            username,
            token,
            channel,
        }
    }

    #[instrument]
    pub async fn run(&self) -> Result<(), Error> {
        info!("Running EgBot");

        loop {
            match self.get_cooldown().await {
                Ok(Some(cooldown)) => {
                    info!("Eg cooldown: {}", cooldown.as_readable());
                    sleep(cooldown).await
                }
                Ok(None) => {
                    trace!("cooldown not active")
                }
                Err(err) => {
                    error!("Could not get cooldown: {:?}", err);

                    sleep(Duration::from_secs(10)).await;
                    continue;
                }
            }

            if !self
                .check_chatters("okayegbot")
                .await
                .map_err(Error::CheckChatters)?
            {
                warn!(
                    "OkayegBOT is not in #{}. Suspending bot for 30 minutes",
                    self.channel
                );
                sleep(Duration::from_secs(60 * 30)).await;
                continue;
            }

            // login to chat server
            let config = ClientConfig::new_simple(StaticLoginCredentials::new(
                self.username.clone(),
                Some(self.token.expose_secret().to_string()),
            ));
            let (mut incoming_messages, client) =
                TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

            client.join(self.channel.clone());

            info!("Claiming egs");
            match self.claim_egs(&client, &mut incoming_messages).await? {
                ClaimEgs::Success {
                    username: _,
                    amount,
                    total,
                } => {
                    info!("Claimed {} egs for a total of {} egs", amount, total);

                    self.wait_for(Duration::from_secs(3600)).await
                }
                ClaimEgs::Failure {
                    username: _,
                    minutes,
                    seconds,
                    total: _,
                } => {
                    warn!("Could not claim egs since cooldown is active");
                    let secs = seconds.unwrap_or(0);
                    let mins = minutes.unwrap_or(0);

                    self.wait_for(Duration::from_secs(secs + mins * 60)).await
                }
            }
        }
    }

    async fn wait_for(&self, duration: Duration) {
        info!("Waiting for {}", duration.as_readable());
        sleep(duration).await;
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn claim_egs(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<ClaimEgs, Error> {
        self.communicate(client, incoming_messages, "=eg")
            .await
            .map_err(Error::Communication)?
            .parse()
            .map_err(Error::ParseClaimEgs)
    }

    async fn get_user_cooldown(&self) -> Result<DateTime<Utc>, Error> {
        let client = self.get_client().map_err(Error::GetClient)?;

        let response: UserResponse = client
            .get("https://api.okayeg.com/user")
            .query(&[("username", &self.username)])
            .send()
            .await
            .map_err(Error::SendRequest)?
            .error_for_status()
            .map_err(Error::BadStatusCode)?
            .json()
            .await
            .map_err(Error::DeserializeResponse)?;

        Ok(response.cooldown)
    }

    async fn get_cooldown(&self) -> Result<Option<Duration>, Error> {
        let last_used = self.get_user_cooldown().await?;
        let now = Utc::now();

        debug!(
            "Server reported cooldown as {}, current time is {}",
            last_used, now
        );

        match last_used
            .checked_add_signed(*CLAIM_EGS_COOLDOWN)
            .expect("Time should not overflow")
            .signed_duration_since(now)
            .to_std()
        {
            Ok(duration) => Ok(Some(duration)),
            Err(_) => Ok(None),
        }
    }
}

impl Bot for EgBot {
    fn accepts_invalid_certs(&self) -> bool {
        false
    }

    fn get_channel(&self) -> &str {
        &self.channel
    }

    fn get_bot_id(&self) -> &str {
        OKAYEG_BOT_USER_ID
    }

    fn get_username(&self) -> &str {
        &self.username
    }

    fn get_generic_answer(&self) -> &regex::Regex {
        &GENERIC_ANSWER
    }
}
