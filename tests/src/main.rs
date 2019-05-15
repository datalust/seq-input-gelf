#[macro_use]
extern crate serde_json;

#[macro_use]
mod support;

mod cases;

fn main() {
    self::cases::test_all();
}
