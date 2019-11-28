#![recursion_limit = "256"]
#![deny(unsafe_code)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

#[macro_use]
pub mod diagnostics;

#[macro_use]
pub mod error;

pub mod config;
pub mod io;
pub mod process;
pub mod receive;
pub mod server;

pub use self::{
    config::Config,
    error::Error,
};
