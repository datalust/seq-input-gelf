mod clef;
mod gelf;
mod str;

use serde_json::Value;

use self::str::{Str, Inlinable, CachedString};

use crate::io::MemRead;

pub type Error = failure::Error;

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
        with: impl FnOnce(clef::Message) -> Result<(), Error>,
    ) -> Result<(), Error> {
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
            ref container_id,
            ref command,
            ref container_name,
            ref created,
            ref image_name,
            ref image_id,
            ref tag,
        } = self;

        let mut clef = clef::Message::maybe_from_json(short_message.as_ref())
            .unwrap_or_else(|| clef::Message::from_message(short_message.as_ref()));

        // Set the log level
        if clef.level.is_none() {
            clef.level = Some(match level.unwrap_or(6) {
                l if l < 3 => Str::Borrowed("Fatal"),
                3 => Str::Borrowed("Error"),
                4 => Str::Borrowed("Warning"),
                l if l < 7 => Str::Borrowed("Info"),
                _ => Str::Borrowed("Debug"),
            })
        }

        // Set the timestamp
        if clef.timestamp.is_none() {
            clef.timestamp = timestamp
                .map(clef::Timestamp::from_float)
                .or_else(|| Some(clef::Timestamp::now()));
        }

        // Set GELF properties
        clef.gelf.host = Some(Str::Borrowed(host.as_ref()));
        clef.gelf.full_message = full_message.as_ref().map(AsRef::as_ref).map(Str::Borrowed);

        // Set the container environment
        clef.docker.container_id = container_id.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.command = command.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.container_name = container_name.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.created = created.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.image_name = image_name.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.image_id = image_id.as_ref().map(AsRef::as_ref).map(Str::Borrowed);
        clef.docker.tag = tag.as_ref().map(AsRef::as_ref).map(Str::Borrowed);

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
                    "@l": "Fatal",
                    "@m": "A short message that helps you identify what is going on",
                    "@t": "2013-11-21T17:11:02.307000000Z",
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

    #[test]
    fn from_gelf_inner_json() {
        let clef = json!({
            "@l": "Info",
            "@m": "A short message that helps you identify what is going on",
            "@t": "2013-11-21T17:11:02Z",
            "user_id": 4000
        });

        let gelf = json!({
            "version": "1.1",
            "host": "example.org",
            "short_message": clef.to_string(),
            "full_message": "Backtrace here",
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
                    "@l": "Info",
                    "@m": "A short message that helps you identify what is going on",
                    "@t": "2013-11-21T17:11:02Z",
                    "some_env_var": "bar",
                    "some_info": "foo",
                    "user_id": 4000,
                    "docker": {
                        "container_id": "abcdefghijklmnopqrstuv",
                        "command": "run",
                        "container_name": "test-container",
                        "image_name": "test/image",
                        "image_id": "abcdefghijklmnopqrstuv",
                        "tag": "latest"
                    },
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
