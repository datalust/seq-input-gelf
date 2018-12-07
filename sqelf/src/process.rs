use std::{
    error, fmt,
    ops::Deref,
    time::{Duration, SystemTime},
};

use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    ser::{Serialize, Serializer},
};

use inlinable_string::{InlineString, INLINE_STRING_CAPACITY};
use serde_json::Value;
use string_cache::DefaultAtom as CachedString;

use crate::io::MemRead;

pub type Error = Box<error::Error + Send + Sync>;

#[derive(Debug)]
pub struct Config {
    pub unprocessed_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            unprocessed_capacity: 1_024,
        }
    }
}

/**
Process a raw message
*/
#[derive(Clone)]
pub struct Process {}

impl Process {
    pub fn new(_: Config) -> Self {
        Process {}
    }

    fn with_clef(
        &self,
        msg: impl MemRead,
        with: impl FnOnce(Clef) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if let Some(bytes) = msg.bytes() {
            let value: Gelf<&str> = serde_json::from_slice(bytes)?;

            with(value.to_clef())
        } else {
            let value: Gelf<Inlinable<CachedString>, String> =
                serde_json::from_reader(msg.into_reader()?)?;

            with(value.to_clef())
        }
    }

    pub fn read_as_clef(&self, msg: impl MemRead) -> Result<(), Error> {
        self.with_clef(msg, |clef| {
            if let Ok(clef) = serde_json::to_string(&clef) {
                println!("{}", clef);
            }

            Ok(())
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Clef<'a> {
    // Clef built-ins
    #[serde(rename = "@m")]
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    #[serde(rename = "@mt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<&'a str>,
    #[serde(rename = "@t")]
    timestamp: Option<Timestamp>,
    #[serde(rename = "@l")]
    level: Option<&'a str>,

    // GELF properties
    #[serde(skip_serializing_if = "ClefGelf::is_empty")]
    gelf: ClefGelf<'a>,

    // Common container properties
    #[serde(skip_serializing_if = "ClefContainer::is_empty")]
    container: ClefContainer<'a>,

    // Everything else
    #[serde(flatten)]
    additional: Option<Value>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ClefGelf<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_message: Option<&'a str>,
}

impl<'a> ClefGelf<'a> {
    fn is_empty(&self) -> bool {
        #![deny(unused_variables)]

        let ClefGelf {
            ref host,
            ref full_message,
        } = self;

        let ops = [host, full_message];

        ops.iter().all(|o| o.is_none())
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ClefContainer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    container_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<&'a str>,
}

impl<'a> ClefContainer<'a> {
    fn is_empty(&self) -> bool {
        #![deny(unused_variables)]

        let ClefContainer {
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
struct Timestamp(SystemTime);

impl Timestamp {
    fn now() -> Self {
        Timestamp(SystemTime::now())
    }

    fn from_float(ts: f64) -> Self {
        let secs = ts.trunc() as u64;
        let millis = ts.fract() as u32;

        Timestamp(SystemTime::UNIX_EPOCH + Duration::new(secs, millis))
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&humantime::format_rfc3339_nanos(self.0))
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

impl<'a> Clef<'a> {
    fn from_message(msg: &'a str) -> Self {
        Clef {
            message: Some(msg),
            template: None,
            timestamp: None,
            gelf: ClefGelf::default(),
            container: ClefContainer::default(),
            additional: None,
            level: None,
        }
    }

    fn maybe_from_json(json: &'a str) -> Option<Self> {
        if json.chars().next() == Some('{') {
            serde_json::from_str(json).ok()
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct Gelf<TString, TMessage = TString> {
    // GELF
    version: TString,
    host: TString,
    short_message: TMessage,
    full_message: Option<TMessage>,
    timestamp: Option<f64>,
    level: Option<u8>,

    // Common Docker parameters
    #[serde(rename = "_container_id")]
    container_id: Option<TString>,
    #[serde(rename = "_command")]
    command: Option<TString>,
    #[serde(rename = "_container_name")]
    container_name: Option<TString>,
    #[serde(rename = "_created")]
    created: Option<TString>,
    #[serde(rename = "_image_name")]
    image_name: Option<TString>,
    #[serde(rename = "_image_id")]
    image_id: Option<TString>,
    #[serde(rename = "_tag")]
    tag: Option<TString>,

    // Everything else
    #[serde(flatten)]
    additional: Option<Value>,
}

impl<TString, TMessage> Gelf<TString, TMessage>
where
    TString: AsRef<str>,
    TMessage: AsRef<str>,
{
    fn to_clef(&self) -> Clef {
        #![deny(unused_variables)]

        let Gelf {
            additional: _additional,
            version: _version,
            ref host,
            ref level,
            ref short_message,
            ref full_message,
            ref timestamp,
            ref container_id,
            ref command,
            ref container_name,
            ref created,
            ref image_name,
            ref image_id,
            ref tag,
        } = self;

        let mut clef = Clef::maybe_from_json(short_message.as_ref())
            .unwrap_or_else(|| Clef::from_message(short_message.as_ref()));

        // Set the log level
        if clef.level.is_none() {
            clef.level = Some(match level.unwrap_or(6) {
                l if l < 3 => "Fatal",
                3 => "Error",
                4 => "Warning",
                l if l < 7 => "Info",
                _ => "Debug",
            })
        }

        // Set the timestamp
        if clef.timestamp.is_none() {
            clef.timestamp = timestamp
                .map(Timestamp::from_float)
                .or_else(|| Some(Timestamp::now()));
        }

        // Set GELF properties
        clef.gelf.host = Some(host.as_ref());
        clef.gelf.full_message = full_message.as_ref().map(AsRef::as_ref);

        // Set the container environment
        clef.container.container_id = container_id.as_ref().map(AsRef::as_ref);
        clef.container.command = command.as_ref().map(AsRef::as_ref);
        clef.container.container_name = container_name.as_ref().map(AsRef::as_ref);
        clef.container.created = created.as_ref().map(AsRef::as_ref);
        clef.container.image_name = image_name.as_ref().map(AsRef::as_ref);
        clef.container.image_id = image_id.as_ref().map(AsRef::as_ref);
        clef.container.tag = tag.as_ref().map(AsRef::as_ref);

        // Set any additional properties
        if let Some(additional) = self.additional() {
            match &mut clef.additional {
                // If the clef message already has properties,
                // merge in the GELF ones (retaining clef)
                Some(Value::Object(ref mut clef)) => {
                    for (k, v) in additional {
                        if !clef.contains_key(k) {
                            clef.insert(k.to_owned(), v.clone());
                        }
                    }
                }
                // If the clef message has no properties,
                // replace them with gelf
                _ => {
                    clef.additional = {
                        let additional = additional
                            .into_iter()
                            .map(|(k, v)| (k.to_owned(), v.to_owned()))
                            .collect();

                        Some(Value::Object(additional))
                    }
                }
            }
        }

        clef
    }

    fn additional(&self) -> Option<impl IntoIterator<Item = (&str, &Value)>> {
        match self.additional {
            Some(Value::Object(ref additional)) => Some(additional.iter().map(|(k, v)| {
                let k = if k.starts_with('_') { &k[1..] } else { &k };

                (k, v)
            })),
            _ => None,
        }
    }
}

enum Inlinable<S> {
    Inline(InlineString),
    Spilled(S),
}

impl<S> fmt::Debug for Inlinable<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Inlinable::Inline(ref s) => {
                let s: &str = &*s;

                f.debug_tuple("Inline").field(&s).finish()
            }
            Inlinable::Spilled(s) => f.debug_tuple("Spilled").field(&s).finish(),
        }
    }
}

impl<S> AsRef<str> for Inlinable<S>
where
    S: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        match self {
            Inlinable::Inline(s) => &*s,
            Inlinable::Spilled(s) => s.as_ref(),
        }
    }
}

impl<S> Deref for Inlinable<S>
where
    S: Deref<Target = str>,
{
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Inlinable::Inline(s) => &*s,
            Inlinable::Spilled(s) => &*s,
        }
    }
}

impl<'de, S> Deserialize<'de> for Inlinable<S>
where
    S: for<'a> From<&'a str>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor<S>(std::marker::PhantomData<S>);

        impl<'de, S> Visitor<'de> for StringVisitor<S>
        where
            S: for<'a> From<&'a str>,
        {
            type Value = Inlinable<S>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > INLINE_STRING_CAPACITY {
                    Ok(Inlinable::Spilled(S::from(value)))
                } else {
                    Ok(Inlinable::Inline(InlineString::from(value)))
                }
            }
        }

        deserializer.deserialize_str(StringVisitor(Default::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn from_gelf_msg() {
        let gelf = br#"{
            "version": "1.1",
            "host": "example.org",
            "short_message": "A short message that helps you identify what is going on",
            "full_message": "Backtrace here",
            "timestamp": 1385053862.3072,
            "level": 1,
            "_user_id": 9001,
            "_some_info": "foo",
            "_some_env_var": "bar"
        }"#;

        let process = Process::new(Default::default());

        process
            .with_clef(gelf as &[u8], |clef| {
                let expected = json!({
                    "@l": "Fatal",
                    "@m": "A short message that helps you identify what is going on",
                    "@t": "2013-11-21T17:11:02.000000000Z",
                    "some_env_var": "bar",
                    "some_info": "foo",
                    "user_id": 9001,
                    "gelf": {
                        "host": "example.org",
                        "full_message": "Backtrace here"
                    }
                });

                let clef = serde_json::to_value(&clef).expect("failed to read clef");

                assert_eq!(expected, clef);

                Ok(())
            })
            .expect("failed to read gelf event");
    }
}
