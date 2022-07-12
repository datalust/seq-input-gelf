#[macro_use]
extern crate serde_json;

#[macro_use]
mod support;
mod cases;

use std::env;

fn main() {
    let case = env::args().skip(1).next();

    if let Some(case) = case {
        cases::test(case);
    } else {
        cases::test_all();
    }
}
