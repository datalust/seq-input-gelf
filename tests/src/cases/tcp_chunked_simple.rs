use crate::support::*;

pub fn test() {
    let mut server = server::tcp();
    let mut stream = tcp::stream();

    stream.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    assert_eq!(0, server.received());

    stream.write(net_chunks![
        ..tcp_delim()
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    stream.close();
    server.close();
}
