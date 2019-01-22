use std::{
    env,
    str::FromStr,
};

use crate::{
    receive,
    process,
    server,
};

pub type Error = failure::Error;

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub receive: receive::Config,
    pub process: process::Config,
    pub server: server::Config,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let mut config = Config::default();

        if is_seq_app() {
            set(&mut config.server.bind, "SEQ_APP_SETTING_GELFADDRESS")?;
        } else {
            set(&mut config.server.bind, "GELF_ADDRESS")?;
        }

        Ok(config)
    }
}

fn is_seq_app() -> bool {
    env::var("SEQ_APP_ID").is_ok()
}

fn set<T>(set: &mut T, name: impl AsRef<str>) -> Result<(), Error>
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
            *set = T::from_str(&v)?;

            Ok(())
        }
    }
}
