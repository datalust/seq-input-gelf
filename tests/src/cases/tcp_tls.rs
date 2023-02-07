use crate::support::server::builder;
use crate::support::*;
use std::path::Path;

pub fn test() {
    if Path::new("127.0.0.1+1.pem").exists() {
        let mut server = builder()
            .tcp_certificate_path("127.0.0.1+1.pem")
            .tcp_certificate_private_key_path("127.0.0.1+1-key.pem")
            .tcp();

        let mut stream = tcp::tls_stream();

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
    } else {
        eprintln!("Ignoring TLS tests because `127.0.0.1+1.pem` doesn't exist. Run `mkcert 127.0.0.1 localhost` to generate a local certificate to test with");
    }
}
