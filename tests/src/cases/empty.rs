use crate::support::*;

pub fn test() {
    expect(
        ToReceive {
            count: 0,
            when_sending: vec![],
        },
        |_| { },
    );
}
