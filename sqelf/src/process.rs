use std::{error, fmt, io::Read, ops::Deref};

use serde::de::{self, Deserialize, Deserializer, Visitor};

use inlinable_string::{InlineString, INLINE_STRING_CAPACITY};
use serde_json::Value;
use string_cache::DefaultAtom as CachedString;

use crate::{io::MemRead, receive};

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
    pub fn new(config: Config) -> Self {
        Process {}
    }

    pub fn read_as_clef(&self, msg: impl MemRead) -> Result<(), Error> {
        if let Some(bytes) = msg.bytes() {
            let mut value: Gelf<&str> = serde_json::from_slice(bytes)?;

            println!("borrowed: {:?}", value);
            println!("clef: {:?}", value.to_clef());
        } else {
            let mut value: Gelf<Inlinable<CachedString>, String> =
                serde_json::from_reader(msg.into_reader()?)?;

            println!("owned: {:?}", value);
            println!("clef: {:?}", value.to_clef());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct Clef<'a> {
    #[serde(rename = "@m")]
    message: &'a str,
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
        Clef {
            message: self
                .full_message
                .as_ref()
                .map(|s| s.as_ref())
                .unwrap_or_else(|| self.short_message.as_ref()),
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
