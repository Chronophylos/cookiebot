use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    pub static ref CLAIM_GOOD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+) -> (?P<cookie>[^!]+)!+ \((?P<amount>[+-±]\d+)\) \w+ \| (?P<total>\d+) total!").unwrap();
    pub static ref CLAIM_BAD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+) you have already claimed a cookie and have (?P<total>\d+) of them!").unwrap();
    pub static ref CD_CHECK_GOOD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+), you have (?P<total>\d+) cookies! 🍪 You can also claim your next cookie now by doing !cookie!").unwrap();
    pub static ref CD_CHECK_BAD: Regex = Regex::new(r"\[Cookies\] \[(?P<level>\w+)\] (?P<username>\w+), you have (?P<total>\d+) cookies! 🍪 (((?P<h>\d) hrs?, )?(?P<m>\d+) mins?, and )?(?P<s>\d+) secs? left until you can claim your next cookie!").unwrap();
    pub static ref BUY_CDR_GOOD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), your cooldown has been reset!").unwrap();
    pub static ref BUY_CDR_BAD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), you can purchase your next cooldown reset in (((?P<h>\d) hrs?, )?(?P<m>\d+) mins?, )?(?P<s>\d+) secs?!").unwrap();
    pub static ref GENERIC_ANSWER: Regex = Regex::new(r"\[\w+\] (\[\w+\])? (?P<username>\w+)").unwrap();
}

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

pub const POSITIVE_BOT_USER_ID: u64 = 425363834;
