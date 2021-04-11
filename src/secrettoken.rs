use secrecy::{CloneableSecret, DebugSecret, Secret, SerializableSecret};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use zeroize::Zeroize;

#[derive(Debug, Clone, Zeroize, Deserialize, Serialize)]
pub struct Token(String);

impl Token {
    pub fn new<S>(s: S) -> Self
    where
        S: ToString,
    {
        Self(s.to_string())
    }
}

impl Deref for Token {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Into<String> for Token {
    fn into(self) -> String {
        self.0
    }
}

impl DebugSecret for Token {}
impl CloneableSecret for Token {}
impl SerializableSecret for Token {}

pub type SecretToken = Secret<Token>;
