use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // https://regex101.com/r/ZQ0SZH/1
    #[derive(Debug)]
    pub static ref CLAIM_GOOD: Regex = Regex::new(r#"\x{1F343} @(?P<username>\w+) > .* \((?P<amount>[+-]\d+)\) \| You've got (?P<total>-?\d+) leaves now! \| Get more leaves in 1 hour\.\.\."#).unwrap();

    // https://regex101.com/r/wuuDX2/1
    #[derive(Debug)]
    pub static ref CLAIM_BAD: Regex = Regex::new(r#"\x{1F343} @(?P<username>\w+) > FeelsBadMan You need to wait (?P<minutes>\d+):(?P<seconds>\d+) minutes until you can get more leaves \| You've got (?P<total>-?\d+) leaves"#).unwrap();

    // https://regex101.com/r/0Fo0Io/1
    #[derive(Debug)]
    pub static ref GENERIC_ANSWER: Regex = Regex::new(r#"\x{1F343} @(?P<username>\w+) > .*"#).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn claim_good() {
        let tests = [("🍃 @chronophylos > Four Leaf Clover 🍀 (+24) | You've got 34 leaves now! | Get more leaves in 1 hour... 🍃 ", ("chronophylos", "+24", "34"))];

        for (text, (username, amount, total)) in tests.iter() {
            let captures = CLAIM_GOOD.captures(text).expect("regex should match");

            assert_eq!(
                captures
                    .name("username")
                    .expect("missing username")
                    .as_str(),
                *username,
                "wrong username"
            );

            assert_eq!(
                captures.name("amount").expect("missing amount").as_str(),
                *amount,
                "wrong amount"
            );

            assert_eq!(
                captures.name("total").expect("missing total").as_str(),
                *total,
                "wrong total"
            );
        }
    }

    #[test]
    fn claim_bad() {
        let tests = [
            ("🍃 @chronophylos > FeelsBadMan You need to wait 45:58 minutes until you can get more leaves | You've got 34 leaves 🍃 ", ("chronophylos", Some("45"), Some("58"), "34")),
            ("🍃 @chronophylos > FeelsBadMan You need to wait 58:08 minutes until you can get more leaves | You've got 34 leaves 🍃 ", ("chronophylos", Some("58"), Some("08"), "34"))
        ];

        for (text, (username, minutes, seconds, total)) in tests.iter() {
            let captures = CLAIM_BAD.captures(text).expect("regex should match");

            assert_eq!(
                captures
                    .name("username")
                    .expect("missing username")
                    .as_str(),
                *username,
                "wrong username"
            );

            assert_eq!(
                captures.name("minutes").map(|m| m.as_str()),
                *minutes,
                "wrong minutes"
            );

            assert_eq!(
                captures.name("seconds").map(|m| m.as_str()),
                *seconds,
                "wrong seconds"
            );

            assert_eq!(
                captures.name("total").expect("missing total").as_str(),
                *total,
                "wrong total"
            );
        }
    }

    #[test]
    fn generic() {
        let tests = [("🍃 @chronophylos > You can find a list of all commands here: https://beatz.dev/leavesbot 🍃 ", "chronophylos")];

        for (text, username) in tests.iter() {
            let captures = GENERIC_ANSWER.captures(text).expect("regex should match");

            assert_eq!(
                captures.name("username").unwrap().as_str(),
                *username,
                "wrong username"
            );
        }
    }
}
