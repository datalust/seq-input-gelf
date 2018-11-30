use std::{error, fmt, io::Read, ops::Deref};

use serde::de::{self, Deserialize, Deserializer, Visitor};

use inlinable_string::{InlineString, INLINE_STRING_CAPACITY};
use serde_json::Value;
use string_cache::DefaultAtom as CachedString;

use crate::receive;

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
pub struct Clef {}

impl Clef {
    pub fn new(config: Config) -> Self {
        Clef {}
    }

    pub fn process(&self, mut msg: impl Read) -> Result<(), Error> {
        let mut value: Message = serde_json::from_reader(msg)?;

        println!("{:?}", value);

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Message {
    // GELF
    version: Inlinable<CachedString>,
    host: Inlinable<CachedString>,
    short_message: Inlinable<String>,
    full_message: Option<Inlinable<String>>,
    timestamp: Option<f64>,
    level: Option<u8>,

    // Common Docker parameters
    #[serde(rename = "_container_id")]
    container_id: Option<Inlinable<CachedString>>,
    #[serde(rename = "_command")]
    command: Option<Inlinable<CachedString>>,
    #[serde(rename = "_container_name")]
    container_name: Option<Inlinable<CachedString>>,
    #[serde(rename = "_created")]
    created: Option<Inlinable<CachedString>>,
    #[serde(rename = "_image_name")]
    image_name: Option<Inlinable<CachedString>>,
    #[serde(rename = "_image_id")]
    image_id: Option<Inlinable<CachedString>>,
    #[serde(rename = "_tag")]
    tag: Option<Inlinable<CachedString>>,

    // Everything else
    #[serde(flatten)]
    additional: Option<Value>,
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
