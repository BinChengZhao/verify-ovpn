use std::time::Duration;
use tokio::net::TcpSocket;
use tokio::net::TcpStream;
use tokio::time::timeout;

pub(crate) async fn verify_addres_by_tcp(address: String) -> bool {
    let sock = TcpSocket::new_v4().unwrap();

    // TODO: unwrap is ugly.
    let stream: TcpStream = sock
        .connect(address.parse().unwrap())
        .await
        .map_err(|e| dbg!(e))
        .unwrap();

    timeout(Duration::from_secs(15), stream.writable())
        .await
        .is_ok()
}
