use std::time::Duration;

use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use serde::Deserialize;
use tokio::{sync::mpsc::UnboundedReceiver, time::sleep};
use tracing::{error, info, instrument, warn};
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not communicate with target bot: {0}")]
    CommunicationError(#[source] bot::Error),

    #[error("Could not parse claim egs message: {0}")]
    ParseClaimEgsError(#[from] ClaimEgsParserError),

    #[error("Could not get client: {0}")]
    GetClientError(#[source] bot::Error),

    #[error("Could not send request: {0}")]
    SendRequestError(#[source] reqwest::Error),

    #[error("Could not deserialize response")]
    DeserializeResponseError(#[source] reqwest::Error),
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
                Ok(Some(cooldown)) => sleep(cooldown).await,
                Ok(None) => {}
                Err(err) => {
                    error!("Could not get cooldown: {:?}", err);

                    sleep(Duration::from_secs(10)).await;
                    continue;
                }
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
            .map_err(|err| Error::CommunicationError(err))?
            .parse()
            .map_err(|err| Error::ParseClaimEgsError(err))
    }

    async fn get_user_cooldown(&self) -> Result<DateTime<Utc>, Error> {
        let client = self
            .get_client()
            .map_err(|err| Error::GetClientError(err))?;

        let response: UserResponse = client
            .get("https://api.okayeg.com/user")
            .query(&[("username", &self.username)])
            .send()
            .await
            .map_err(|err| Error::SendRequestError(err))?
            .json()
            .await
            .map_err(|err| Error::DeserializeResponseError(err))?;

        Ok(response.cooldown)
    }

    async fn get_cooldown(&self) -> Result<Option<Duration>, Error> {
        let cooldown = self.get_user_cooldown().await?;
        let now = Utc::now();

        match cooldown.signed_duration_since(now).to_std() {
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
