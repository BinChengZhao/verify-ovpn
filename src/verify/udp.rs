use std::fmt::Debug;
use std::io::Result;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio::time::timeout;

pub(crate) async fn verify_addres_by_udp(address: impl ToSocketAddrs + Debug) -> Result<usize> {
    // change to 0.0.0.0:0 is good.
    // bind() the socket before you send your data. Specify port 0 to bind(),
    // and the OS will pick an unused port for you.

    let sock = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap()).await?;
    sock.set_ttl(15)?;

    sock.connect(address).await.unwrap_or(());

    sock.send(b"hello world").await?;
    let mut buf = vec![0; 10];
    timeout(Duration::from_secs(15), sock.recv(&mut buf)).await?
}
