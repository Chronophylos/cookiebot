use super::constants::*;
use crate::{twitch::connect, Timestamp, Toggle};
use anyhow::{anyhow, bail, Context, Result};
use log::{debug, info, trace, warn};
use regex::Regex;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use smol::{future::FutureExt, Timer};
use std::{
    borrow::Cow, fmt::Display, num::ParseIntError, ops::Deref, str::FromStr, time::Duration,
};
use thiserror::Error;
use twitchchat::{commands, messages, AsyncRunner, Status, UserConfig};

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Rank {
    Default,
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Masters,
    GrandMasters,
    Leader,
}

impl Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Default => "default",
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
            Self::Platinum => "platinum",
            Self::Diamond => "diamond",
            Self::Masters => "masters",
            Self::GrandMasters => "grandmasters",
            Self::Leader => "leader",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
enum ParseRankError {
    #[error("unknown rank name")]
    UnkownRankError,
}

impl FromStr for Rank {
    type Err = ParseRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "bronze" => Ok(Self::Bronze),
            "silver" => Ok(Self::Silver),
            "gold" => Ok(Self::Gold),
            "platinum" => Ok(Self::Platinum),
            "diamond" => Ok(Self::Diamond),
            "masters" => Ok(Self::Masters),
            "grandmasters" => Ok(Self::GrandMasters),
            "leader" => Ok(Self::Leader),
            _ => Err(Self::Err::UnkownRankError),
        }
    }
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

#[derive(Debug, Error)]
enum ParsePresigeRankError {
    #[error("Missing character P")]
    MissingP,

    #[error("Missing prestige part before :")]
    MissingPrestigePart,

    #[error("Missing rank part after :")]
    MissingRankPart,

    #[error("Could not parse prestige")]
    ParsePrestigeError(#[source] ParseIntError),

    #[error("Could not parse rank")]
    ParseRankError(#[source] ParseRankError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrestigeRank {
    prestige: u32,
    rank: Rank,
}

impl Display for PrestigeRank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{}: {}", self.prestige, self.rank)
    }
}

impl FromStr for PrestigeRank {
    type Err = ParsePresigeRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("P").ok_or(Self::Err::MissingP)?;
        let mut split = s.split(':');

        let prestige = split
            .next()
            .ok_or(Self::Err::MissingPrestigePart)?
            .parse()
            .map_err(|e| Self::Err::ParsePrestigeError(e))?;

        let rank = split
            .next()
            .ok_or(Self::Err::MissingRankPart)?
            .trim()
            .parse()
            .map_err(|e| Self::Err::ParseRankError(e))?;

        Ok(PrestigeRank { prestige, rank })
    }
}

/// Result of a claim cookie command
#[derive(Debug, Clone, PartialEq, Eq)]
enum ClaimCookieResponse {
    /// Command was successful
    Success {
        rank: PrestigeRank,
        name: String,
        amount: i32,
        total: u64,
    },

    /// Command is on cooldown
    Cooldown { rank: PrestigeRank, total: u64 },
}

#[derive(Debug, Error)]
enum ClaimCookieError {
    #[error("Regex match is missing named capture group {0}")]
    MissingCaptureGroup(&'static str),

    #[error("Could not parse prestige and rank")]
    ParsePrestigeRankError(#[from] ParsePresigeRankError),

    #[error("Could not parse int")]
    ParseIntError(#[from] ParseIntError),

    #[error("Input did not match regex")]
    InvalidInput,
}

impl FromStr for ClaimCookieResponse {
    type Err = ClaimCookieError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(captures) = CLAIM_GOOD.captures(s) {
            let rank = captures
                .name("rank")
                .ok_or(Self::Err::MissingCaptureGroup("rank"))?
                .as_str()
                .parse()?;

            let name = captures
                .name("cookie")
                .map(|m| m.as_str())
                .ok_or(Self::Err::MissingCaptureGroup("cookie"))?
                .to_string();

            let amount = captures
                .name("amount")
                .ok_or(Self::Err::MissingCaptureGroup("amount"))?
                .as_str()
                .trim_start_matches('±')
                .parse()?;

            let total = captures
                .name("total")
                .ok_or(Self::Err::MissingCaptureGroup("total"))?
                .as_str()
                .parse()?;

            Ok(Self::Success {
                rank,
                name,
                amount,
                total,
            })
        } else if let Some(captures) = CLAIM_BAD.captures(s) {
            let rank = captures
                .name("rank")
                .ok_or(Self::Err::MissingCaptureGroup("rank"))?
                .as_str()
                .parse()?;

            let total = captures
                .name("total")
                .ok_or(Self::Err::MissingCaptureGroup("total"))?
                .as_str()
                .parse()?;

            Ok(Self::Cooldown { rank, total })
        } else {
            Err(Self::Err::InvalidInput)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClaimCookieResponse, PrestigeRank, Rank};

    #[test]
    fn parse_claimcookie() {
        let input = "[Cookies] [P6: default] chronophylos you have already claimed a cookie and have 4957 of them! 🍪 Please wait in 2 hour intervals! ";
        let response = input.parse::<ClaimCookieResponse>().unwrap();

        assert_eq!(
            response,
            ClaimCookieResponse::Cooldown {
                rank: PrestigeRank {
                    prestige: 6,
                    rank: Rank::Default
                },
                total: 4957
            }
        )
    }
}

#[derive(Debug)]
pub struct Bot {
    user_config: UserConfig,
    channel: String,
    runner: AsyncRunner,
    send_byte: bool,
    booster_mode: bool,
    accept_invalid_certs: bool,
}

async fn connect_and_retry(user_config: &UserConfig, channel: &str) -> Result<AsyncRunner> {
    loop {
        match connect(user_config, channel).await {
            Ok(runner) => return Ok(runner),
            Err(twitchchat::RunnerError::UnexpectedEof) => continue,
            Err(err) => return Err(err).context("Could not connect to twitch"),
        }
    }
}

impl Bot {
    pub async fn new(
        user_config: UserConfig,
        channel: String,
        booster_mode: bool,
        enable_ssl: bool,
    ) -> Result<Self> {
        let runner = connect_and_retry(&user_config, &channel).await?;

        Ok(Self {
            user_config,
            channel,
            runner,
            send_byte: false,
            booster_mode,
            accept_invalid_certs: enable_ssl,
        })
    }

    pub async fn main_loop(&mut self) -> Result<()> {
        loop {
            self.wait_for_cooldown().await?;

            match self.claim_cookies().await? {
                ClaimCookieResponse::Success {
                    rank: _,
                    name,
                    amount,
                    total,
                } => {
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

                    if self.booster_mode {
                        unreachable!("booster are disabled");
                    //if total >= 300 {
                    //    if !self.buy_booster().await? {
                    //        warn!("Could not buy booster")
                    //    }
                    //}
                    } else {
                        if total >= 5000 {
                            if !self.prestige().await? {
                                warn!(
                                    "Could not upgrade prestige but cookie count is over 5000 ({})",
                                    total
                                );
                            }
                        }
                    }

                    info!("Waiting for cooldown");
                }
                ClaimCookieResponse::Cooldown { rank: _, total: _ } => {
                    info!("Could not claim cookies: Cooldown active");
                }
            }
        }
    }

    async fn wait_for_cooldown(&mut self) -> Result<()> {
        info!("Checking cookie cooldown");

        if let Some(duration) = self.get_cookie_cd()? {
            info!("Cooldown active");

            debug!("Terminating twitch connection");
            self.runner.quit_handle().notify().await;

            info!("Waiting for {}", duration.as_readable());
            Timer::after(duration).await;

            debug!("Restoring twitch connection");
            self.reconnect().await?;
        } else {
            info!("Cooldown not active")
        }

        Ok(())
    }

    fn get_cookie_cd(&mut self) -> Result<Option<Duration>> {
        let client = reqwest::blocking::Client::builder()
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
            .context("Could not send request to api.roaringiron.com")?
            .json()
            .context("Could not deserialize json response")?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        if response.can_claim {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs_f32(response.seconds_left)))
        }
    }

    /*
    fn get_booster_cd(&mut self) -> Result<Option<Duration>> {
        let client = reqwest::blocking::Client::new();
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
            .send()?
            .json()?;

        debug!("Got response from api.roaringiron.com: {:?}", response);

        if response.can_claim {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs_f32(response.seconds_left)))
        }

        todo!()
    }
    */

    async fn claim_cookies(&mut self) -> Result<ClaimCookieResponse> {
        info!("Claiming cookies");

        self.communicate("!cookie")
            .await?
            .parse()
            .context("Could not parse response of cookie command")
    }

    async fn prestige(&mut self) -> Result<bool> {
        self.request("!prestige", &PRESTIGE_GOOD, &PRESTIGE_BAD)
            .await
    }

    async fn buy_cdr(&mut self) -> Result<bool> {
        self.request("!cdr", &BUY_CDR_GOOD, &BUY_CDR_BAD).await
    }

    //async fn buy_booster(&mut self) -> Result<bool> {
    //    self.request("!shop purchase globalbooster", &BUY_CDR_GOOD, &BUY_CDR_BAD)
    //        .await
    //}

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

    async fn request<ReGood, ReBad>(
        &mut self,
        message: &str,
        re_good: &ReGood,
        re_bad: &ReBad,
    ) -> Result<bool>
    where
        ReGood: Deref<Target = Regex>,
        ReBad: Deref<Target = Regex>,
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
