use crate::timestamp::Timestamp;
use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, FROM, USER_AGENT};
use std::time::Duration;
use tokio::{
    sync::mpsc::UnboundedReceiver,
    time::{sleep, timeout},
};
use tracing::{debug, info, instrument, trace};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, TCPTransport, TwitchIRCClient,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not build request client: {0}")]
    BuildReqwestClientError(#[source] reqwest::Error),

    #[error("Could not parse header value: {0}")]
    ParsingHeaderValueError(#[source] reqwest::header::InvalidHeaderValue),

    #[error("Could not authenticate with the chat server")]
    AuthenticateChatError,

    #[error("Did not receive a message from the chat server")]
    ReceivedNoMessageError,

    #[error("Could not communicate with chat server after {0} attempts")]
    FailedCommunicationError(u32),

    #[error("Could not send message to chat: {0}")]
    SendingMessageError(#[source] twitch_irc::Error<TCPTransport, StaticLoginCredentials>),

    #[error("No Regex Pattern matched the provided message")]
    NoMatchingRegex,
}

#[async_trait]
pub trait Bot {
    /// Returns weather invalid certificates should be accepted by the bot.
    fn accepts_invalid_certs(&self) -> bool;

    /// Returns a refrence to the channel where the bot should sit.
    fn get_channel(&self) -> &str;

    /// Returns the user id of the bot to talk with.
    fn get_bot_id(&self) -> &str;

    /// Returns the username of the bot.
    fn get_username(&self) -> &str;

    /// Returns a regex matching a generic answer by the target bot.
    ///
    /// This is used to ensure the target bot is talking to us.
    fn get_generic_answer(&self) -> &Regex;

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
            .danger_accept_invalid_certs(self.accepts_invalid_certs())
            .default_headers(headers)
            .build()
            .map_err(|err| Error::BuildReqwestClientError(err))
    }

    #[instrument(skip(self, incoming_messages))]
    async fn wait_for_answer(
        &self,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Result<String, Error> {
        debug!("Waiting for response");

        while let Some(server_message) = incoming_messages.recv().await {
            trace!("received message: {:?}", &server_message);

            match server_message {
                ServerMessage::Privmsg(msg) => {
                    if msg.sender.id != self.get_bot_id() {
                        trace!("UserID not matching");
                        continue;
                    }

                    if let Some(captures) = self.get_generic_answer().captures(&msg.message_text) {
                        let matched_username = captures
                            .name("username")
                            .expect("could not get username")
                            .as_str();

                        if matched_username == self.get_username() {
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
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
        message: &str,
    ) -> Result<String, Error> {
        const MAX_RETRIES: u32 = 3;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying communication: Retry {}", retry)
            }

            let message_to_send = if retry % 2 == 0 {
                format!("{}\u{E0000}", message)
            } else {
                message.to_string()
            };

            client
                .say(self.get_channel().to_string(), message_to_send)
                .await
                .map_err(|err| Error::SendingMessageError(err))?;

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

    #[instrument(skip(self, client, incoming_messages))]
    async fn request(
        &self,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
        incoming_messages: &mut UnboundedReceiver<ServerMessage>,
        message: &str,
        re_good: Regex,
        re_bad: Regex,
    ) -> Result<bool, Error> {
        let response = self.communicate(client, incoming_messages, message).await?;

        if re_good.is_match(&response) {
            Ok(true)
        } else if re_bad.is_match(&response) {
            Ok(false)
        } else {
            Err(Error::NoMatchingRegex)
        }
    }
}
