use std::io::Result;
use std::time::Duration;
use tokio::net::TcpSocket;
use tokio::time::timeout;

pub(crate) async fn verify_addres_by_tcp(address: String) -> Result<()> {
    let sock = TcpSocket::new_v4()?;

    let stream = sock.connect(address.parse().unwrap()).await?;

    timeout(Duration::from_secs(15), stream.writable()).await?
}
