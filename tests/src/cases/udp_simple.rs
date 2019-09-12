use crate::support::*;

pub fn test() {
    let mut server = server::udp();
    let mut sock = udp::sock();

    sock.send(net_chunks![
        ..net_chunks!({
            "host": "foo",
            "short_message": "bar"
        })
    ]);

    server.receive(|received| {
        assert_eq!("bar", received["@m"]);
    });

    assert_eq!(1, server.received());

    server.close();
}
