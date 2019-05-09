mod clef;
mod gelf;
mod str;

use serde_json::Value;

use self::str::{CachedString, Inlinable, Str};

use crate::{error::Error, io::MemRead};

use std::collections::HashMap;

metrics! {
    msg
}

/**
Configuration for CELF formatting.
*/
#[derive(Debug, Clone)]
pub struct Config {}

impl Default for Config {
    fn default() -> Self {
        Config {}
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
#[derive(Clone)]
pub struct Process {}

impl Process {
    pub fn new(_: Config) -> Self {
        Process {}
    }

    fn with_clef(
        &self,
        msg: impl MemRead,
        with: impl FnOnce(clef::Message) -> Result<(), Error>,
    ) -> Result<(), Error> {
        increment!(process.msg);

        if let Some(bytes) = msg.bytes() {
            let value: gelf::Message<Str> = serde_json::from_slice(bytes)?;

            with(value.to_clef())
        } else {
            let value: gelf::Message<Inlinable<CachedString>, String> =
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

        // Set the timestamp
        if clef.timestamp.is_none() {
            clef.timestamp = timestamp
                .map(clef::Timestamp::from_float)
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

        clef
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
                    "@t": "2013-11-21T17:11:02.307000000Z",
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
}
