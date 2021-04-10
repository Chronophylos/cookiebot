use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    #[derive(Debug)]
    pub static ref CLAIM_GOOD: Regex = Regex::new(r"\[Cookies\] \[(?P<rank>(P\d+: )?\w+)\] (?P<username>\w+) -> (?P<cookie>[^!]+)!+ \((?P<amount>[+-±]\d+)\) \w+ \| (?P<total>\d+) total!").unwrap();
    #[derive(Debug)]
    pub static ref CLAIM_BAD: Regex = Regex::new(r"\[Cookies\] \[(?P<rank>(P\d+: )?\w+)\] (?P<username>\w+) you have already claimed a cookie and have (?P<total>\d+) of them!").unwrap();

    #[derive(Debug)]
    pub static ref BUY_CDR_GOOD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), your cooldown has been reset!").unwrap();
    #[derive(Debug)]
    pub static ref BUY_CDR_BAD: Regex = Regex::new(r"\[Shop\] (?P<username>\w+), you can purchase your next cooldown reset in (((?P<h>\d) hrs?, )?(?P<m>\d+) mins?, )?(?P<s>\d+) secs?!").unwrap();

    #[derive(Debug)]
    pub static ref PRESTIGE_GOOD: Regex = Regex::new(r"\[Cookies\] (?P<username>\w+) you reset your rank and are now \[(?P<rank>(P\d: )?\w+)\]!").unwrap();
    #[derive(Debug)]
    pub static ref PRESTIGE_BAD: Regex = Regex::new(r"\[Cookies\] (?P<username>\w+) you are not ranked high enough to Prestige yet! FeelsBadMan You need Leader rank OR 5000\+ cookies!").unwrap();

    #[derive(Debug)]
    pub static ref GENERIC_ANSWER: Regex = Regex::new(r"\[(Cookies|Shop)\]( \[(?P<rank>(P\d+: )?\w+)\])? (?P<username>\w+)").unwrap();
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

        assert_eq!(captures.name("rank").unwrap().as_str(), "default");
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

        assert_eq!(captures.name("rank").unwrap().as_str(), "Gold");
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

        assert_eq!(captures.name("rank").unwrap().as_str(), "Silver");
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
    fn claim_good4() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [P1: default] chronophylos -> Sugar cookie! (+14) PJSugar | 65 total! | 2 hour cooldown... 🍪"
            )
            .expect("regex should match");

        assert_eq!(captures.name("rank").unwrap().as_str(), "P1: default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("cookie").unwrap().as_str(), "Sugar cookie");
        assert_eq!(captures.name("amount").unwrap().as_str(), "+14");
        assert_eq!(captures.name("total").unwrap().as_str(), "65");
    }

    #[test]
    fn claim_good5() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [P1: default] chronophylos -> Raisin cookie! (-6) DansGame | 79 total! | 2 hour cooldown... 🍪"
            )
            .expect("regex should match");

        assert_eq!(captures.name("rank").unwrap().as_str(), "P1: default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("cookie").unwrap().as_str(), "Raisin cookie");
        assert_eq!(captures.name("amount").unwrap().as_str(), "-6");
        assert_eq!(captures.name("total").unwrap().as_str(), "79");
    }

    #[test]
    fn claim_good6() {
        let captures = CLAIM_GOOD.captures(
            "[Cookies] [P10: default] chronophylos -> Raisin cookie! (-6) DansGame | 79 total! | 2 hour cooldown... 🍪"
            )
            .expect("regex should match");

        assert_eq!(captures.name("rank").unwrap().as_str(), "P10: default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("cookie").unwrap().as_str(), "Raisin cookie");
        assert_eq!(captures.name("amount").unwrap().as_str(), "-6");
        assert_eq!(captures.name("total").unwrap().as_str(), "79");
    }

    #[test]
    fn claim_bad1() {
        let captures = CLAIM_BAD.captures(
            "[Cookies] [default] chronophylos you have already claimed a cookie and have 31 of them! 🍪 Please wait in 2 hour intervals!"
            )
            .expect("regex should match");

        assert_eq!(captures.name("rank").unwrap().as_str(), "default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("total").unwrap().as_str(), "31");
    }

    #[test]
    fn claim_bad2() {
        let captures = CLAIM_BAD.captures(
            "[Cookies] [P1: default] chronophylos you have already claimed a cookie and have 65 of them! 🍪 Please wait in 2 hour intervals!"
            )
            .expect("regex should match");

        assert_eq!(captures.name("rank").unwrap().as_str(), "P1: default");
        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("total").unwrap().as_str(), "65");
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

    #[test]
    fn prestige_good() {
        let captures = PRESTIGE_GOOD
            .captures("[Cookies] chronophylos you reset your rank and are now [P1: default]! PartyHat PogChamp The next rank is Bronze (50 🍪 )! Have fun climbing back up :)")
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(captures.name("rank").unwrap().as_str(), "P1: default");
    }

    #[test]
    fn prestige_bad() {
        let captures = PRESTIGE_BAD
            .captures("[Cookies] chronophylos you are not ranked high enough to Prestige yet! FeelsBadMan You need Leader rank OR 5000+ cookies!")
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
    }

    #[test]
    fn generic_answer1() {
        let captures = GENERIC_ANSWER
            .captures("[Cookies] chronophylos you are not ranked high enough to Prestige yet! FeelsBadMan You need Leader rank OR 5000+ cookies!")
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
    }

    #[test]
    fn generic_answer2() {
        let captures = GENERIC_ANSWER
            .captures("[Cookies] [P1: default] chronophylos you have already claimed a cookie and have 65 of them! 🍪 Please wait in 2 hour intervals!")
            .expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
    }
}
