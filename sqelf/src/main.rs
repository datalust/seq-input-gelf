#[macro_use]
extern crate serde_derive;

mod diagnostics;
pub mod io;
pub mod process;
pub mod receive;
pub mod server;

mod config;

pub use self::config::Config;
use self::diagnostics::emit_err;

use std::error;

fn main() {
    if let Err(err) = run() {
        emit_err(&err, "Server initialization failed");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<error::Error>> {
    let config = Config::from_env()?;

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
