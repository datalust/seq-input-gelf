pub mod clef;
mod gelf;
pub mod str;

use serde_json::Value;

use self::str::{
    CachedString,
    Inlinable,
    Str,
};

use crate::{
    Error,
    io::MemRead,
};

use std::{
    collections::HashMap,
    io::Read,
};

metrics! {
    msg
}

/**
Configuration for CELF formatting.
*/
#[derive(Debug, Clone)]
pub struct Config {
    /**
    Whether or not to buffer and include the raw GELF payload
    in the event message.
    */
    pub include_raw_payload: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            include_raw_payload: false,
        }
    }
}

/**
Build a CLEF processor to handle messages.
*/
pub fn build(config: Config) -> Process {
    Process::new(config)
}

/**
Process a raw message
*/
#[derive(Debug, Clone)]
pub struct Process {
    include_raw_payload: bool,
}

impl Process {
    pub fn new(config: Config) -> Self {
        Process {
            include_raw_payload: config.include_raw_payload,
        }
    }

    pub fn with_clef(
        &self,
        msg: impl MemRead,
        with: impl FnOnce(clef::Message) -> Result<(), Error>,
    ) -> Result<(), Error> {
        increment!(process.msg);

        if let Some(bytes) = msg.bytes() {
            let value = if self.include_raw_payload {
                let mut value: gelf::Message<Str> = serde_json::from_slice(bytes)
                    .map_err(Error::from)
                    .map_err(|e| e.context(format!("could not parse GELF from: {:?}", String::from_utf8_lossy(bytes))))?;

                value.add(
                    "raw_payload",
                    Value::String(String::from_utf8_lossy(bytes).into_owned()),
                );

                value
            } else {
                serde_json::from_slice(bytes)?
            };

            with(value.to_clef())
        } else {
            let value = if self.include_raw_payload {
                let mut payload = String::new();
                msg.into_reader()?.read_to_string(&mut payload)?;

                let mut value: gelf::Message<Inlinable<CachedString>, String> =
                    serde_json::from_str(&payload)
                    .map_err(Error::from)
                    .map_err(|e| e.context(format!("could not parse GELF from: {:?}", payload)))?;

                value.add("raw_payload", Value::String(payload));

                value
            } else {
                serde_json::from_reader(msg.into_reader()?)?
            };

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

impl<TString, TMessage> gelf::Message<TString, TMessage>
where
    TString: AsRef<str>,
    TMessage: AsRef<str>,
{
    /**
    Covert a GELF message into CLEF.

    The contents of the GELF message is inspected and deserialized as CLEF-encoded
    JSON if possible. In this case, timestamp, message, and level information from
    the embedded CLEF is given precedence over the outer GELF envelope.

    Other fields with conflicting names are prioritized:

      GELF envelope > GELF payload > Embedded CLEF/JSON

    This means fields set by the system/on the logger are preferred over
    the fields attached to any one event.

    If fields conflict, then the lower-priority field is included with a
    double-underscore-prefixed name, e.g.: "__host".
    */
    fn to_clef(&self) -> clef::Message {
        #![deny(unused_variables)]

        let gelf::Message {
            additional: _additional,
            version: _version,
            ref host,
            ref level,
            ref short_message,
            ref full_message,
            ref timestamp,
            ref facility,
            ref file,
            ref line,
        } = self;

        let mut clef = clef::Message::maybe_from_json(short_message.as_ref())
            .unwrap_or_else(|| clef::Message::from_message(short_message.as_ref()));

        // Set the log level; these are the standard Syslog levels
        if clef.level.is_none() {
            clef.level = Some(match level.unwrap_or(6) {
                0 => Str::Borrowed("emerg"),
                1 => Str::Borrowed("alert"),
                2 => Str::Borrowed("crit"),
                3 => Str::Borrowed("err"),
                4 => Str::Borrowed("warning"),
                5 => Str::Borrowed("notice"),
                6 => Str::Borrowed("info"),
                7 => Str::Borrowed("debug"),
                _ => Str::Borrowed("debug"),
            })
        }

        // Set the timestamp, giving priority to the embedded CLEF timestamp
        if clef.timestamp.is_none() {
            clef.timestamp = timestamp
                .and_then(clef::Timestamp::from_decimal)
                .or_else(|| Some(clef::Timestamp::now()));
        }

        // Set the exception, giving priority to the embedded CLEF exception.
        if clef.exception.is_none() {
            clef.exception = full_message
                .as_ref()
                .map(AsRef::as_ref)
                // If the full message is the same as the short message then don't
                // bother setting it. Some clients will defensively send the same
                // value in both fields.
                .filter(|full_message| *full_message != short_message.as_ref())
                .map(Str::Borrowed);
        }

        // Set additional properties first; these override any in an embedded CLEF payload,
        // because we trust the configuration of the logger ahead of any one event.
        if let Some(additional) = self.additional() {
            for (k, v) in additional {
                Self::override_value(&mut clef.additional, k, v.clone());
            }
        }

        // Set GELF built-in properties; we also trust these ahead of any one event's properties.
        if let Some(host) = host {
            Self::override_value(
                &mut clef.additional,
                "host",
                host.as_ref().to_string().into(),
            );
        }

        if let Some(facility) = facility {
            Self::override_value(
                &mut clef.additional,
                "facility",
                facility.as_ref().to_string().into(),
            );
        }

        if let Some(file) = file {
            Self::override_value(
                &mut clef.additional,
                "file",
                file.as_ref().to_string().into(),
            );
        }

        if let Some(line) = line {
            Self::override_value(&mut clef.additional, "line", (*line).into());
        }

        // If we reach the end without a message or message template then try find a
        // suitable substitute in the events properties
        if clef.message.is_none() && clef.message_template.is_none() {
            clef.message = Self::find_first(&clef.additional, &["message", "msg"])
                .and_then(|msg| match msg.as_str() {
                    Some(str) => Some(Str::Owned(str.to_owned())),
                    None => None
                });
        }

        clef
    }

    fn find_first<'a, 'b>(fields: &'b HashMap<Str<'a>, Value>, names: &'b [&str]) -> Option<&'b Value> {
        for name in names {
            if let Some(value) = fields.get(&Str::Borrowed(name)) {
                return Some(value)
            }
        }

        None
    }

    fn override_value<'a>(
        fields: &mut HashMap<Str<'a>, Value>,
        name: &'a (impl AsRef<str> + ?Sized),
        value: Value,
    ) {
        if let Some(old) = fields.insert(Str::Borrowed(name.as_ref()), value) {
            fields.insert(Str::Owned(format!("__{}", name.as_ref())), old);
        }
    }

    fn add(&mut self, k: &str, v: Value) -> bool {
        use serde_json::map::Entry;

        match self.additional {
            Some(Value::Object(ref mut additional)) => {
                if let Entry::Vacant(vacant) = additional.entry(k) {
                    vacant.insert(v);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn from_gelf_msg() {
        let gelf = json!({
            "version": "1.1",
            "host": "example.org",
            "short_message": "A short message that helps you identify what is going on",
            "full_message": "Backtrace here",
            "timestamp": 1385053862.3072,
            "level": 1,
            "_user_id": 9001,
            "_some_info": "foo",
            "_some_env_var": "bar"
        });

        let process = Process::new(Default::default());

        process
            .with_clef(gelf.to_string().as_bytes(), |clef| {
                if let Str::Owned(_) = clef.message.as_ref().expect("missing message") {
                    panic!("expected a borrowed message string");
                }

                let expected = json!({
                    "@t": "2013-11-21T17:11:02.307200000Z",
                    "@l": "alert",
                    "@m": "A short message that helps you identify what is going on",
                    "@x": "Backtrace here",
                    "some_env_var": "bar",
                    "some_info": "foo",
                    "user_id": 9001,
                    "host": "example.org",
                });

                let clef = serde_json::to_value(&clef).expect("failed to read clef");

                assert_eq!(expected, clef);

                Ok(())
            })
            .expect("failed to read gelf event");
    }

    #[test]
    fn from_gelf_inner_json() {
        let clef = json!({
            "@l": "info",
            "@mt": "A short message that helps {user_id} identify what is going on",
            "@t": "2013-11-21T17:11:02Z",
            "@x": "Backtrace here",
            "user_id": 4000
        });

        let gelf = json!({
            "version": "1.1",
            "host": "example.org",
            "short_message": clef.to_string(),
            "level": 1,
            "_user_id": 9001,
            "_some_info": "foo",
            "_some_env_var": "bar",
            "_container_id": "abcdefghijklmnopqrstuv",
            "_command": "run",
            "_container_name": "test-container",
            "_image_name": "test/image",
            "_image_id": "abcdefghijklmnopqrstuv",
            "_tag": "latest"
        });

        let process = Process::new(Default::default());

        process
            .with_clef(gelf.to_string().as_bytes(), |clef| {
                let expected = json!({
                    "@l": "info",
                    "@mt": "A short message that helps {user_id} identify what is going on",
                    "@t": "2013-11-21T17:11:02Z",
                    "@x": "Backtrace here",
                    "some_env_var": "bar",
                    "some_info": "foo",
                    "user_id": 9001,
                    "__user_id": 4000,
                    "container_id": "abcdefghijklmnopqrstuv",
                    "command": "run",
                    "container_name": "test-container",
                    "image_name": "test/image",
                    "image_id": "abcdefghijklmnopqrstuv",
                    "tag": "latest",
                    "host": "example.org"
                });

                let clef = serde_json::to_value(&clef).expect("failed to read clef");

                assert_eq!(expected, clef);

                Ok(())
            })
            .expect("failed to read gelf event");
    }

    #[test]
    fn from_gelf_inner_json_fallback() {
        let clef = json!({
            "message": "A short message that helps {user_id} identify what is going on"
        });

        let gelf = json!({
            "version": "1.1",
            "timestamp": 1385053862.3072,
            "host": "example.org",
            "short_message": clef.to_string(),
            "level": 6,
        });

        let process = Process::new(Default::default());

        process
            .with_clef(gelf.to_string().as_bytes(), |clef| {
                let expected = json!({
                    "@l": "info",
                    "@m": "A short message that helps {user_id} identify what is going on",
                    "@t": "2013-11-21T17:11:02.307200000Z",
                    "host": "example.org",
                    "message": "A short message that helps {user_id} identify what is going on"
                });

                let clef = serde_json::to_value(&clef).expect("failed to read clef");

                assert_eq!(expected, clef);

                Ok(())
            })
            .expect("failed to read gelf event");
    }

    #[test]
    fn include_raw_payload() {
        let gelf = json!({
            "version": "1.1",
            "host": "example.org",
            "short_message": "A short message that helps you identify what is going on",
            "full_message": "Backtrace here",
            "timestamp": 1385053862.04736,
            "level": 1,
            "_user_id": 9001,
            "_some_info": "foo",
            "_some_env_var": "bar"
        });

        let process = Process::new(Config { include_raw_payload: true, ..Default::default() });

        process
            .with_clef(gelf.to_string().as_bytes(), |clef| {
                if let Str::Owned(_) = clef.message.as_ref().expect("missing message") {
                    panic!("expected a borrowed message string");
                }

                let expected = json!({
                    "@t": "2013-11-21T17:11:02.047360000Z",
                    "@l": "alert",
                    "@m": "A short message that helps you identify what is going on",
                    "@x": "Backtrace here",
                    "some_env_var": "bar",
                    "some_info": "foo",
                    "user_id": 9001,
                    "host": "example.org",
                    "raw_payload": gelf.to_string()
                });

                let clef = serde_json::to_value(&clef).expect("failed to read clef");

                assert_eq!(expected, clef);

                Ok(())
            })
            .expect("failed to read gelf event");
    }

    #[test]
    fn invalid_json_includes_some_raw_content() {
        let gelf = "this is definitely not json";

        let process = Process::new(Config { include_raw_payload: true, ..Default::default() });

        let err = process.with_clef(gelf.as_bytes(), |_| unreachable!()).expect_err("expected parsing to fail");

        assert!(err.to_string().contains(gelf));
    }
}
