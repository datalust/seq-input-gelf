use crate::support::*;

pub fn test() {
    tcp_expect(
        ToReceive {
            count: 1,
            when_sending: net_chunks![
                ..net_chunks!({
                    "host": "foo",
                    "short_message": "bar"
                }),
                ..tcp_delim()
            ],
        },
        |received| {
            assert_eq!("bar", received[0]["@m"]);
        },
    );
}
