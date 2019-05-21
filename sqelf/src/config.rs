use std::{env, str::FromStr};

use crate::{diagnostics, process, receive, server, Error};

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub receive: receive::Config,
    pub process: process::Config,
    pub server: server::Config,
    pub diagnostics: diagnostics::Config,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let mut config = Config::default();
        let is_seq_app = is_seq_app();

        let bind_address_var = if is_seq_app {
            "SEQ_APP_SETTING_GELFADDRESS"
        } else {
            "GELF_ADDRESS"
        };
        read_environment(&mut config.server.bind, bind_address_var)?;

        let enable_diagnostics = if is_seq_app {
            "SEQ_APP_SETTING_ENABLEDIAGNOSTICS"
        } else {
            "GELF_ENABLE_DIAGNOSTICS"
        };
        if is_truthy(enable_diagnostics)? {
            config.diagnostics.min_level = diagnostics::Level::Debug;
        }

        Ok(config)
    }
}

pub(crate) fn is_seq_app() -> bool {
    env::var("SEQ_APP_ID").is_ok()
}

fn is_truthy(name: impl AsRef<str>) -> Result<bool, Error> {
    match env::var(name.as_ref()) {
        // The evironment variable contains a truthy value
        Ok(ref v) if v == "True" || v == "true" => return Ok(true),
        // The environment variable is not set or doesn't contain
        // a truthy value
        Ok(_) | Err(env::VarError::NotPresent) => return Ok(false),
        // The environment variable is invalid
        Err(e) => Err(e)?,
    }
}

fn read_environment<T>(into: &mut T, name: impl AsRef<str>) -> Result<(), Error>
where
    T: FromStr,
    Error: From<T::Err>,
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
