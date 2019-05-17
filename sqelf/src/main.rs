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
    error::{err_msg, Error},
};

use std::{io::Read, panic::catch_unwind, thread};

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
    let mut server = server::build(config.server, receive, process)?;

    // If we should listen for stdin to terminate
    if config::is_seq_app() {
        let handle = server
            .take_handle()
            .ok_or_else(|| err_msg("Failed to acquire handle to server"))?;

        listen_for_stdin_closed(handle);
    }

    // Run the server and wait for it to exit
    server.run()?;
    diagnostics.stop_metrics()?;

    Ok(())
}

fn listen_for_stdin_closed(handle: server::Handle) {
    // NOTE: This is a regular thread instead of `tokio`
    // so that we don't block with our synchronous read that
    // will probably never actually return
    thread::spawn(move || 'wait: loop {
        match std::io::stdin().read(&mut [u8::default()]) {
            Ok(0) => {
                let _ = handle.close();
                break 'wait;
            }
            Ok(_) => {
                continue 'wait;
            }
            Err(_) => {
                let _ = handle.close();
                break 'wait;
            }
        }
    });
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
