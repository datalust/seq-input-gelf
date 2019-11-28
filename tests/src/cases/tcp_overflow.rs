use std::str;

use crate::support::*;

pub fn test() {
    let mut server = server::tcp();
    let mut stream = tcp::stream();

    let short_message = str::from_utf8(&[b'a'; 1024]).unwrap();

    stream.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": short_message
        }),
        ..tcp_delim()
    ]);

    stream.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        }),
        ..tcp_delim()
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    assert_eq!(1, server.received());

    stream.close();
    server.close();
}
