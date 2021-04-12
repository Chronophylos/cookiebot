use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // https://regex101.com/r/6gc79V/3
    #[derive(Debug)]
    pub static ref CLAIM_GOOD: Regex = Regex::new(r#"@(?P<username>\w+) \| [^\|]* \| (?P<amount>[+-]\d+) +egs \| Total egs: (?P<total>\d+) 🥚"#).unwrap();

    // https://regex101.com/r/g4FpOL/1/
    #[derive(Debug)]
    pub static ref CLAIM_BAD: Regex = Regex::new(r#"@(?P<username>\w+) nam1Sadeg no eg. come back in (?P<minutes>\d+) minutes?,( (?P<seconds>\d+) seconds?)? Total egs: (?P<total>\d+)"#).unwrap();

    // https://regex101.com/r/GaJODf/1/
    #[derive(Debug)]
    pub static ref GENERIC_ANSWER: Regex = Regex::new(r#"@(?P<username>\w+) .*"#).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn claim_good1() {
        let text = "@chronophylos | viseit babushka ni Borovits, giv 14 eg ad ohme med spirtis vodak regard ov dedushka nam1Okayeg | +14 egs | Total egs: 30 🥚";
        let captures = CLAIM_GOOD.captures(text).expect("regex should match");

        dbg!(&captures);

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(
            captures.name("amount").unwrap().as_str(),
            "+14",
            "wrong eg changed amount"
        );
        assert_eq!(
            captures.name("total").unwrap().as_str(),
            "30",
            "wrong total eg amount"
        );
    }

    #[test]
    fn claim_good2() {
        let text = "@chronophylos | is this a YOLK? nam1Okayeg | +1 egs | Total egs: 92 🥚 ";
        let captures = CLAIM_GOOD.captures(text).expect("regex should match");

        dbg!(&captures);

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(
            captures.name("amount").unwrap().as_str(),
            "+1",
            "wrong eg changed amount"
        );
        assert_eq!(
            captures.name("total").unwrap().as_str(),
            "92",
            "wrong total eg amount"
        );
    }

    #[test]
    fn claim_good_with_weird_space() {
        let text = "@chronophylos | is this a YOLK? nam1Okayeg | +1  egs | Total egs: 92 🥚 ";
        let captures = CLAIM_GOOD.captures(text).expect("regex should match");

        dbg!(&captures);

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(
            captures.name("amount").unwrap().as_str(),
            "+1",
            "wrong eg changed amount"
        );
        assert_eq!(
            captures.name("total").unwrap().as_str(),
            "92",
            "wrong total eg amount"
        );
    }

    #[test]
    fn claim_bad1() {
        let text =
            "@chronophylos nam1Sadeg no eg. come back in 56 minutes, 42 seconds Total egs: 30";
        let captures = CLAIM_BAD.captures(text).expect("regex should match");

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(
            captures.name("minutes").unwrap().as_str(),
            "56",
            "wrong minutes"
        );
        assert_eq!(
            captures.name("seconds").unwrap().as_str(),
            "42",
            "wrong seconds"
        );
        assert_eq!(
            captures.name("total").unwrap().as_str(),
            "30",
            "wrong total eg amount"
        );
    }

    #[test]
    fn claim_bad2() {
        let text = "@chronophylos nam1Sadeg no eg. come back in 50 minutes, Total egs: 30";
        let captures = CLAIM_BAD.captures(text).expect("regex should match");

        dbg!(&captures);

        assert_eq!(
            captures.name("username").unwrap().as_str(),
            "chronophylos",
            "wrong username"
        );
        assert_eq!(
            captures.name("minutes").unwrap().as_str(),
            "50",
            "wrong minutes"
        );
        assert!(
            captures.name("seconds").is_none(),
            "seconds should not exist"
        );
        assert_eq!(
            captures.name("total").unwrap().as_str(),
            "30",
            "wrong total eg amount"
        );
    }

    #[test]
    fn generic_answer1() {
        let tests = [(
            "@chronophylos nam1Sadeg no eg. come back in 50 minutes, Total egs: 30",
            "chronophylos",
        )];

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
