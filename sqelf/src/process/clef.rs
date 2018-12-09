use std::{
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
pub(super) struct Message<'a> {
    // Clef built-ins
    #[serde(rename = "@m")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) message: Option<Str<'a>>,
    #[serde(rename = "@mt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) template: Option<Str<'a>>,
    #[serde(rename = "@t")]
    pub(super) timestamp: Option<Timestamp>,
    #[serde(rename = "@l")]
    #[serde(borrow)]
    pub(super) level: Option<Str<'a>>,

    // GELF properties
    #[serde(skip_serializing_if = "Gelf::is_empty")]
    #[serde(default)]
    pub(super) gelf: Gelf<'a>,

    // Common container properties
    #[serde(skip_serializing_if = "Docker::is_empty")]
    #[serde(default)]
    pub(super) docker: Docker<'a>,

    // Everything else
    #[serde(flatten)]
    pub(super) additional: Option<Value>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub(super) struct Gelf<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) host: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) full_message: Option<Str<'a>>,
}

impl<'a> Gelf<'a> {
    pub(super) fn is_empty(&self) -> bool {
        #![deny(unused_variables)]

        let Gelf {
            ref host,
            ref full_message,
        } = self;

        let ops = [host, full_message];

        ops.iter().all(|o| o.is_none())
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub(super) struct Docker<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) container_id: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) command: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) container_name: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) created: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) image_name: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) image_id: Option<Str<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    pub(super) tag: Option<Str<'a>>,
}

impl<'a> Docker<'a> {
    fn is_empty(&self) -> bool {
        #![deny(unused_variables)]

        let Docker {
            ref container_id,
            ref command,
            ref container_name,
            ref created,
            ref image_name,
            ref image_id,
            ref tag,
        } = self;

        let ops = [
            container_id,
            command,
            container_name,
            created,
            image_name,
            image_id,
            tag,
        ];

        ops.iter().all(|o| o.is_none())
    }
}

#[derive(Debug)]
pub(super) struct Timestamp(SystemTime);

impl Timestamp {
    pub(super) fn now() -> Self {
        Timestamp(SystemTime::now())
    }

    pub(super) fn from_float(ts: f64) -> Self {
        // If the timestamp is before the epoch
        // then just return the epoch
        if ts.is_sign_negative() {
            return Timestamp(SystemTime::UNIX_EPOCH)
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
            message: Some(Str::Borrowed(msg)),
            template: None,
            timestamp: None,
            gelf: Gelf::default(),
            docker: Docker::default(),
            additional: None,
            level: None,
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