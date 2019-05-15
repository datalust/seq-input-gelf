#![allow(dead_code)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
mod diagnostics;

#[macro_use]
pub mod error;

pub mod io;
pub mod process;
pub mod receive;
pub mod server;

mod config;

pub use self::{config::Config, error::Error};
