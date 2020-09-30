#![forbid(unsafe_code)]

mod config;
mod timestamp;
mod toggle;

use anyhow::{anyhow, bail, Context, Result};
use config::Config;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use regex::{Captures, Regex};
use serde::Deserialize;
use smol::{future::FutureExt, Timer};
use std::time::Duration;
use timestamp::Timestamp;
use toggle::Toggle;
use twitchchat::{
    commands, connector, messages, twitch::Capability, AsyncRunner, Status, UserConfig,
};

lazy_static! {
    static ref CONFIG: Config = Config::from_path("cookiebot.ron").unwrap();
    static ref CLAIM_GOOD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+) -> (?P<cookie>[^!]+)!+ \((?P<amount>[+-±]\d+)\) \w+ \| (?P<total>\d+) total!").unwrap();
    static ref CLAIM_BAD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+) you have already claimed a cookie and have (?P<total>\d+) of them!").unwrap();
    static ref CD_CHECK_GOOD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+), you have (?P<total>\d+) cookies! 🍪 You can also claim your next cookie now by doing !cookie!").unwrap();
    static ref CD_CHECK_BAD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+), you have (?P<total>\d+) cookies! 🍪 (((?P<h>\d) hrs?, )?(?P<m>\d+) mins?, and )?(?P<s>\d+) secs? left until you can claim your next cookie!").unwrap();
    static ref BUY_CDR_GOOD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), your cooldown has been reset!").unwrap();
    static ref BUY_CDR_BAD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), you can purchase your next cooldown reset in (((?P<h>\d) hrs?, )?(?P<m>\d+) mins?, )?(?P<s>\d+) secs?!").unwrap();
    static ref GENERIC_ANSWER: Regex = Regex::new(r"\[\w+\] (\[\w+\])? (?P<username>\w+)").unwrap();
}

const POSITIVE_BOT_USER_ID: u64 = 425363834;

/*
macro_rules! gen_capture_fun {
    ($re_left:expr, $re_right:expr, $name:ident, $msg:literal, $left:ty, $right:ty) => {
        async fn $name(&mut self) -> Result<Either<$left, $right>> {
            let answer = self.communicate($msg).await?;


            if let Some(captures) = $re_left.captures(&answer) {
                for name in $re_left.capture_names() {

                }
            }


            Err(anyhow!("no regex matched"))
        }
    };
}
*/

#[cfg(test)]
mod test_regex {
    use super::*;

    #[test]
    fn claim_good1() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [default] chronophylos -> Chocolate Chip! (+6) PartyTime | 31 total! | 2 hour cooldown... 🍪"
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("cookie").unwrap().as_str(), "Chocolate Chip");
        assert_eq!(captures.name("amount").unwrap().as_str(), "+6");
        assert_eq!(captures.name("total").unwrap().as_str(), "31");
    }

    #[test]
    fn claim_good2() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [Gold] fewo11 -> Cinnamon Roll cookie! (+16) OpieOP | 49 total! | 2 hour cooldown... 🍪"
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "Gold");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "fewo11",
            "wrong username"
        );
        assert_eq!(
            captures.name("cookie").unwrap().as_str(),
            "Cinnamon Roll cookie"
        );
        assert_eq!(captures.name("amount").unwrap().as_str(), "+16");
        assert_eq!(captures.name("total").unwrap().as_str(), "49");
    }

    #[test]
    fn claim_good3() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [Silver] efdev -> Nothing Found!! (±0) RPGEmpty | 84 total! | 2 hour cooldown... 🍪 "
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "Silver");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "efdev",
            "wrong username"
        );
        assert_eq!(captures.name("cookie").unwrap().as_str(), "Nothing Found");
        assert_eq!(captures.name("amount").unwrap().as_str(), "±0");
        assert_eq!(captures.name("total").unwrap().as_str(), "84");
    }

    #[test]
    fn claim_bad() {
        let captures = CLAIM_BAD.captures(
            "[Cookies] [default] chronophylos you have already claimed a cookie and have 31 of them! 🍪 Please wait in 2 hour intervals!"
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("total").unwrap().as_str(), "31");
    }

    #[test]
    fn cd_check_bad1() {
        let captures = CD_CHECK_BAD.captures(
            "[Cookies] [default] chronophylos, you have 31 cookies! 🍪 1 hr, 59 mins, and 33 secs left until you can claim your next cookie!"
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("total").unwrap().as_str(), "31");
        assert_eq!(captures.name("h").unwrap().as_str(), "1");
        assert_eq!(captures.name("m").unwrap().as_str(), "59");
        assert_eq!(captures.name("s").unwrap().as_str(), "33");
    }

    #[test]
    fn cd_check_bad2() {
        let captures = CD_CHECK_BAD.captures(
            "[Cookies] [Gold] fewo11, you have 33 cookies! 🍪 34 mins, and 29 secs left until you can claim your next cookie!"
            )
            .expect("regex should match");

        assert_eq!(captures.name("level").unwrap().as_str(), "Gold");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "fewo11",
            "wrong username"
        );
        assert_eq!(captures.name("total").unwrap().as_str(), "33");
        assert!(captures.name("h").is_none());
        assert_eq!(captures.name("m").unwrap().as_str(), "34");
        assert_eq!(captures.name("s").unwrap().as_str(), "29");
    }

    #[test]
    fn buy_cdr_good() {
        let captures = BUY_CDR_GOOD
            .captures(
                "[Shop] chronophylos, your cooldown has been reset! (-7) Good Luck... ThankEgg",
            )
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
    }

    #[test]
    fn buy_cdr_bad() {
        let captures = BUY_CDR_BAD
            .captures("[Shop] chronophylos, you can purchase your next cooldown reset in 2 hrs, 58 mins, 54 secs!")
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("h").unwrap().as_str(), "2");
        assert_eq!(captures.name("m").unwrap().as_str(), "58");
        assert_eq!(captures.name("s").unwrap().as_str(), "54");
    }
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

struct Bot {
    user_config: UserConfig,
    channel: String,
    runner: AsyncRunner,
    send_byte: bool,
}

impl Bot {
    pub async fn new(user_config: UserConfig, channel: String) -> Result<Self> {
        let runner = connect(&user_config, &channel).await?;

        Ok(Self {
            user_config,
            channel,
            runner,
            send_byte: false,
        })
    }

    pub async fn main_loop(&mut self) -> Result<()> {
        self.wait_for_cooldown().await?;

        loop {
            info!("Claiming cookies");
            if let Some((cookie, amount, _total)) = self.claim_cookies().await? {
                if amount == 0 {
                    info!("No cookies found");
                } else {
                    info!("Got {} {}s", amount, cookie);
                }

                #[cfg(shop)]
                if amount > 7 {
                    info!("Trying to buy cooldown reduction for 7 cookies");
                    if self.buy_cdr().await? {
                        info!("Cooldown was reset");
                        continue;
                    }
                }

                info!("Sleeping for 2 hours");
                smol::Timer::after(Duration::from_secs(2 * 60 * 60)).await;
            } else {
                info!("Could not claim cookies: Cooldown active");

                self.wait_for_cooldown().await?;
            }
        }
    }

    async fn wait_for_cooldown(&mut self) -> Result<()> {
        info!("Checking cookie cooldown");
        match self.get_cookie_cd().await? {
            None => {
                info!("Cooldown not active");
            }
            Some(duration) => {
                info!("Current cooldown. Waiting for {}", duration.as_readable());
                smol::Timer::after(duration).await;
            }
        }

        Ok(())
    }

    async fn get_cookie_cd(&mut self) -> Result<Option<Duration>> {
        let client = reqwest::Client::new();
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
            .send()
            .await?
            .json()
            .await?;

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

    //gen_capture_fun!(CLAIM_GOOD, CLAIM_BAD, claim_cookies, "!cookie");

    #[cfg(shop)]
    async fn buy_cdr(&mut self) -> Result<bool> {
        let msg = self.communicate("!shop buy cooldownreset").await?;

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
        const MAX_RETRIES: u64 = 3;

        for retry in 0..MAX_RETRIES {
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
                    // back off after time out
                    let sleep = Duration::from_secs(1 + retry.pow(2));
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

async fn connect(user_config: &UserConfig, channel: &str) -> anyhow::Result<AsyncRunner> {
    // create a connector using ``smol``, this connects to Twitch.
    // you can provide a different address with `custom`
    // this can fail if DNS resolution cannot happen
    let connector = connector::SmolConnectorTls::twitch()?;

    info!("Connecting to twitch");
    // create a new runner. this is a provided async 'main loop'
    // this method will block until you're ready
    let mut runner = AsyncRunner::connect(connector, user_config).await?;

    info!("Connected with Identity: {:#?}", runner.identity);

    info!("Attempting to join '{}'", channel);
    runner.join(channel).await?;
    info!("Joined '{}'!", channel);

    Ok(runner)
}

fn total_from_captures(captures: Captures) -> Result<u64> {
    let total = captures
        .name("total")
        .map(|m| m.as_str().parse::<u64>())
        .context("could not get total")?
        .context("could not parse total")?;

    Ok(total)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let user_config = UserConfig::builder()
        .name(&CONFIG.username)
        .token(&CONFIG.token)
        .capabilities(&[Capability::Tags])
        .build()?;

    Bot::new(user_config, CONFIG.channel.clone())
        .await?
        .main_loop()
        .await
}
