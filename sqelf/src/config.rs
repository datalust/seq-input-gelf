use std::{env, str::FromStr};

use crate::{Error, process, receive, server};

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub receive: receive::Config,
    pub process: process::Config,
    pub server: server::Config,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let mut config = Config::default();

        let is_seq_app = is_seq_app();
        config.server.wait_on_stdin = is_seq_app;

        let bind_address_var = if is_seq_app {
            "SEQ_APP_SETTING_GELFADDRESS"
        } else {
            "GELF_ADDRESS"
        };

        read_environment(&mut config.server.bind, bind_address_var)?;

        Ok(config)
    }
}

fn is_seq_app() -> bool {
    env::var("SEQ_APP_ID").is_ok()
}

fn read_environment<T>(into: &mut T, name: impl AsRef<str>) -> Result<(), Error>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(name.as_ref()) {
        // The environment variable exists, but is empty
        Ok(ref v) if v == "" => return Ok(()),
        // The environment variable does not exist
        Err(env::VarError::NotPresent) => return Ok(()),
        // The environment variable is invalid
        Err(e) => Err(e)?,
        // The environment variable has a value
        Ok(v) => {
            *into = T::from_str(&v)?;

            Ok(())
        }
    }
}
