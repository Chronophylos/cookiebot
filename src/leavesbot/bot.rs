use std::time::Duration;

use lazy_static::lazy_static;
use secrecy::ExposeSecret;
use tokio::{
    sync::mpsc::UnboundedReceiver,
    time::{sleep, sleep_until, Instant},
};
use tracing::{info, instrument, warn};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, TCPTransport,
    TwitchIRCClient,
};

use crate::{
    bot::{self, Bot},
    leavesbot::parser::ClaimResponse,
    SecretToken, Timestamp,
};

use super::{parser::ClaimResponseParserError, patterns::GENERIC_ANSWER};

const COOLDOWN_COST: f32 = 8.;
const MULTIPLIER_COST: f32 = 24.;
const THRESHOLD_COST_MULTIPLIER: f32 = 1.5;
static USER_ID: &str = "731132488";
static USER_NAME: &str = "leavesbot";
static CLAIM_MESSAGE: &str = "*leaves";
static CDR_MESSAGE: &str = "*cdr";
static MULTIPLIER_MESSAGE: &str = "*multiplier";
lazy_static! {
    static ref CLAIM_COOLDOWN: Duration = Duration::from_secs(3600);
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not check chatters: {0}")]
    CheckChattersError(#[source] bot::Error),

    #[error("Could not communicate with target bot: {0}")]
    CommunicationError(#[source] bot::Error),

    #[error("Could not parse claim egs message: {0}")]
    ParseClaimResponse(#[from] ClaimResponseParserError),
}

#[derive(Debug)]
pub struct LeafBot {
    username: String,
    token: SecretToken,
    channel: String,
}

impl Bot for LeafBot {
    fn accepts_invalid_certs(&self) -> bool {
        false
    }

    fn get_channel(&self) -> &str {
        &self.channel
    }

    fn get_bot_id(&self) -> &str {
        USER_ID
    }

    fn get_username(&self) -> &str {
        &self.username
    }

    fn get_generic_answer(&self) -> &regex::Regex {
        &GENERIC_ANSWER
    }
}

impl LeafBot {
    pub fn new(username: String, token: SecretToken, channel: String) -> Self {
        Self {
            username,
            token,
            channel,
        }
    }

    #[instrument]
    pub async fn run(&self) -> Result<(), Error> {
        info!("Running LeafBot");

        loop {
            // check if the bot is online
            if !self
                .check_chatters(USER_NAME)
                .await
                .map_err(Error::CheckChattersError)?
            {
                warn!(
                    "LeavesBot is not in #{}. Suspending bot for 30 minutes",
                    self.channel
                );
                sleep(Duration::from_secs(60 * 30)).await;
                continue;
            }

            // login to tmi
            let (mut incoming_messages, client) = self.login();

            // try claiming leaves
            let amount = match self.claim(&client, &mut incoming_messages).await? {
                ClaimResponse::Success { amount, total, .. } => {
                    info!("Claimed {} leaves for a total of {} leaves", amount, total);

                    amount as f32
                }
                ClaimResponse::Cooldown {
                    minutes, seconds, ..
                } => {
                    warn!("Could not claim leaves since cooldown is active");
                    let secs = seconds.unwrap_or(0);
                    let mins = minutes.unwrap_or(0);

                    self.wait_for(Duration::from_secs(secs + mins * 60)).await;
                    continue;
                }
            };

            let cooldown_deadline = Instant::now() + *CLAIM_COOLDOWN;

            // buy cooldown reduction or multiplier
            if amount >= (COOLDOWN_COST * THRESHOLD_COST_MULTIPLIER) {
                // wait 5 seconds before sending command
                // buy cooldown
            }

            if amount >= (COOLDOWN_COST + MULTIPLIER_COST * THRESHOLD_COST_MULTIPLIER) {
                // wait 5 seconds before sending command
                // buy multiplier
            }

            // wait 1 hour
            self.wait_until(cooldown_deadline).await
        }
    }

    async fn wait_for(&self, duration: Duration) {
        info!("Waiting for {}", duration.as_readable());
        sleep(duration).await;
    }

    async fn wait_until(&self, deadline: Instant) {
        info!("Waiting until {:?}", deadline);
        sleep_until(deadline).await;
    }

    #[instrument]
    fn login(
        &self,
    ) -> (
        UnboundedReceiver<ServerMessage>,
        TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) {
        let config = ClientConfig::new_simple(StaticLoginCredentials::new(
            self.username.clone(),
            Some(self.token.expose_secret().to_string()),
        ));
        let (incoming_messages, client) =
            TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

        client.join(self.channel.clone());

        (incoming_messages, client)
    }

    #[instrument(skip(self, client, incoming_messages))]
    async fn claim(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<ClaimResponse, Error> {
        self.communicate(client, incoming_messages, CLAIM_MESSAGE)
            .await
            .map_err(Error::CommunicationError)?
            .parse()
            .map_err(Error::ParseClaimResponse)
    }
}
