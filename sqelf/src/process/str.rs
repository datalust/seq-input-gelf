use std::{
    cmp::{Ord, Ordering, PartialOrd},
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    ser::{Serialize, Serializer},
};

use inlinable_string::{InlineString, INLINE_STRING_CAPACITY};

pub use string_cache::DefaultAtom as CachedString;

/**
A specialized `Cow<'a, str>` that can be deserialized using
borrowed data.
*/
#[derive(Debug)]
pub enum Str<'a, S = String> {
    Borrowed(&'a str),
    Owned(S),
}

impl<'a, S> AsRef<str> for Str<'a, S>
where
    S: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        match self {
            Str::Borrowed(s) => s,
            Str::Owned(ref s) => s.as_ref(),
        }
    }
}

impl<'a, 'b, SA, SB> PartialEq<Str<'b, SB>> for Str<'a, SA>
where
    SA: AsRef<str>,
    SB: AsRef<str>,
{
    fn eq(&self, other: &Str<'b, SB>) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<'a, S> Eq for Str<'a, S> where Str<'a, S>: PartialEq {}

impl<'a, S> Hash for Str<'a, S>
where
    S: AsRef<str>,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.as_ref().hash(state)
    }
}

impl<'a, 'b, SA, SB> PartialOrd<Str<'b, SB>> for Str<'a, SA>
where
    SA: AsRef<str>,
    SB: AsRef<str>,
{
    fn partial_cmp(&self, other: &Str<'b, SB>) -> Option<Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}

impl<'a, S> Ord for Str<'a, S>
where
    S: AsRef<str>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl<'a, T> Serialize for Str<'a, T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Str::Borrowed(s) => serializer.serialize_str(s),
            Str::Owned(ref s) => s.serialize(serializer),
        }
    }
}

impl<'de: 'a, 'a, S> Deserialize<'de> for Str<'a, S>
where
    S: From<String>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor<'a, S>(std::marker::PhantomData<Str<'a, S>>);

        impl<'de: 'a, 'a, S> Visitor<'de> for StringVisitor<'a, S>
        where
            S: From<String>,
        {
            type Value = Str<'a, S>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Str::Owned(S::from(value.to_owned())))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Str::Borrowed(value))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Str::Owned(S::from(value)))
            }
        }

        deserializer.deserialize_str(StringVisitor(Default::default()))
    }
}

/**
A string that might be stored inline, or elsewhere.
*/
#[derive(Debug)]
pub(super) enum Inlinable<S> {
    Inline(InlineString),
    Spilled(S),
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
