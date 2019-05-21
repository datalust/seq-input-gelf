use crate::support::*;

pub fn test() {
    expect(
        ToReceive {
            count: 1,
            when_sending: dgrams![
                ..dgrams!({
                    "host": "foo",
                    "short_message": "bar"
                })
            ],
        },
        |received| {
            assert_eq!("bar", received[0]["@m"]);
        },
    );
}
