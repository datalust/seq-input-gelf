use crate::support::*;

pub fn test() {
    let mut server = server::tcp();
    let mut stream1 = tcp::stream();
    let mut stream2 = tcp::stream();

    stream1.write(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    stream2.write(net_chunks![
        ..tcp_delim()
    ]);

    assert_eq!(0, server.received());

    stream1.close();
    stream2.close();
    server.close();
}
