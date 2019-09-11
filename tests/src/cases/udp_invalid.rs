use crate::support::*;

pub fn test() {
    let mut server = server::udp();
    let mut sock = udp::sock();

    sock.send(net_chunks![
        ..bytes(b"not json!")
    ]);

    assert_eq!(0, server.received());

    sock.send(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    server.close();
}
