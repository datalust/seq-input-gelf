use crate::support::*;

pub fn test() {
    udp_expect(
        ToReceive {
            count: 0,
            when_sending: vec![],
        },
        |_| { },
    );
}
