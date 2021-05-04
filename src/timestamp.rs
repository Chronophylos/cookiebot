use std::time::Duration;

pub trait Timestamp {
    fn as_readable(&self) -> String;
}

impl Timestamp for Duration {
    fn as_readable(&self) -> String {
        let mut time = Vec::new();
        let mut secs = self.as_secs();

        for (name, s) in &[("h", 60 * 60), ("m", 60), ("s", 1)] {
            let div = secs / s;
            if div > 0 {
                time.push(format!("{}{}", div, name));
                secs -= s * div;
            }
        }

        time.join(" ")
    }
}
