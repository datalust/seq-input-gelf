use std::{error, io::Read};

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
        let mut buf = String::new();
        msg.read_to_string(&mut buf)?;

        // TODO: Actually process into clef format
        println!("{}", buf);

        Ok(())
    }
}
