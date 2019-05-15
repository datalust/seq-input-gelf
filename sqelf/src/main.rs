#[macro_use]
extern crate serde_derive;

#[macro_use]
mod diagnostics;

#[macro_use]
mod error;

mod config;
pub mod io;
pub mod process;
pub mod receive;
pub mod server;

use self::{
    config::Config,
    diagnostics::{emit, emit_err},
    error::Error,
};

use std::panic::catch_unwind;

fn run() -> Result<(), error::StdError> {
    let config = Config::from_env()?;

    // Initialize diagnostics
    let mut diagnostics = diagnostics::init(config.diagnostics);

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
    let (server, handle) = server::build(config.server, receive, process)?;

    if handle.is_some() {
        bail!("In-process handles aren't supported when running as a standalone server");
    }

    // Run the server and wait for it to exit
    server.run()?;
    diagnostics.stop_metrics()?;

    Ok(())
}

fn main() {
    let run_server: Result<(), error::StdError> = catch_unwind(|| run())
        .map_err(|panic| error::unwrap_panic(panic).into())
        .and_then(|inner| inner);

    if let Err(err) = run_server {
        emit_err(&err, "GELF input failed");
        std::process::exit(1);
    }

    emit("GELF input stopped");
}
