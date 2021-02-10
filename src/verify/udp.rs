use std::fmt::Debug;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio::time::timeout;

pub(crate) async fn verify_addres_by_udp(address: impl ToSocketAddrs + Debug) -> bool {
    // change to 0.0.0.0:0 is good.
    // bind() the socket before you send your data. Specify port 0 to bind(),
    // and the OS will pick an unused port for you.

    // TODO: unwrap is ugly.
    let sock = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let f = |e| println!("{}", e);

    sock.set_ttl(15).unwrap_or_else(f);

    sock.connect(address)
        .await
        .map_err(|e| dbg!(e))
        .unwrap_or(());

    sock.send(b"hello world").await.unwrap_or(0);
    let mut buf = vec![0; 10];
    timeout(Duration::from_secs(15), sock.recv(&mut buf))
        .await
        .is_ok()
}
