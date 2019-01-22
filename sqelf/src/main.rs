#[macro_use]
extern crate serde_derive;

pub mod io;
pub mod process;
pub mod receive;
pub mod server;
mod diagnostics;

use std::error;

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub receive: receive::Config,
    pub process: process::Config,
    pub server: server::Config,
}

impl Config {
    fn get() -> Self {
        // NOTE: We'll want to read config from the env
        Config::default()
    }
}

fn main() -> Result<(), Box<error::Error>> {
    let config = Config::get();

    // The receiver for GELF messages
    let receive = {
        let mut receive = receive::build(config.receive);
        move |src| receive.decode(src)
    };

    // The processor for converting GELF into CLEF
    let process = {
        let process = process::build(config.process);
        move |msg| process.read_as_clef(msg)
    };

    // The server that drives the receiver and processor
    let server = server::build(config.server, receive, process)?;

    tokio::run(server);

    Ok(())
}
