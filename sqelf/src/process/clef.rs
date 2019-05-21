use std::{
    collections::HashMap,
    fmt,
    time::{Duration, SystemTime},
};

use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    ser::{Serialize, Serializer},
};

use serde_json::Value;

use super::str::Str;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message<'a> {
    #[serde(rename = "@t")]
    pub timestamp: Option<Timestamp>,

    #[serde(rename = "@l")]
    #[serde(borrow)]
    pub level: Option<Str<'a>>,

    #[serde(rename = "@m")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub message: Option<Str<'a>>,

    #[serde(rename = "@mt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub message_template: Option<Str<'a>>,

    // This is mapped from `full_message`, which GELF suggests might contain a backtrace
    #[serde(rename = "@x")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub exception: Option<Str<'a>>,

    // @i and @r are currently not implemented

    // Everything else
    #[serde(flatten)]
    pub additional: HashMap<Str<'a>, Value>,
}

#[derive(Debug)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    pub(super) fn now() -> Self {
        Timestamp(SystemTime::now())
    }

    pub(super) fn from_float(ts: f64) -> Self {
        // If the timestamp is before the epoch
        // then just return the epoch
        if ts.is_sign_negative() {
            return Timestamp(SystemTime::UNIX_EPOCH);
        }

        let secs = ts.trunc() as u64;
        let nanos = {
            let nanos = (ts.fract() * 10f64.powi(9)) as u32;
            (nanos / 1_000_000) * 1_000_000
        };

        Timestamp(SystemTime::UNIX_EPOCH + Duration::new(secs, nanos))
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&humantime::format_rfc3339(self.0))
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor;

        impl<'de> Visitor<'de> for StringVisitor {
            type Value = Timestamp;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an RFC3339 formatted string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let ts = humantime::parse_rfc3339(value).map_err(|e| E::custom(e))?;

                Ok(Timestamp(ts))
            }
        }

        deserializer.deserialize_str(StringVisitor)
    }
}

impl<'a> Message<'a> {
    pub(super) fn from_message(msg: &'a str) -> Self {
        Message {
            timestamp: None,
            level: None,
            message: Some(Str::Borrowed(msg)),
            message_template: None,
            exception: None,
            additional: Default::default(),
        }
    }

    pub(super) fn maybe_from_json(json: &'a str) -> Option<Self> {
        if json.chars().next() == Some('{') {
            serde_json::from_str(json).ok()
        } else {
            None
        }
    }
}
