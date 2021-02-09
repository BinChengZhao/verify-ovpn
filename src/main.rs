use std::env::set_current_dir;
use std::fmt::Debug;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use std::{ffi::OsString, path::PathBuf};
use structopt::StructOpt;
use tokio::fs::{read_dir, File};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::{TcpSocket, ToSocketAddrs, UdpSocket};
use tokio::runtime::Runtime;
use tokio::task::spawn;
use tokio::time::timeout;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "verify-ovpn")]
struct Opt {
    // A flag, true if used in the command line. Note doc comment will
    // be used for the help message of the flag. The name of the
    // argument will be, by default, based on the name of the field.
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Set speed
    #[structopt(short, long, default_value = "1000")]
    speed: f64,

    /// Input file
    #[structopt(short, long, parse(from_os_str))]
    input: PathBuf,

    /// Output file
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    /// admin_level to consider
    #[structopt(short, long)]
    level: Vec<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Proto {
    Udp,
    Tcp,
}

#[derive(Debug, Clone, Default)]
struct ConcurrencyLimits {
    inner: Arc<Mutex<Option<Waker>>>,
}

impl Future for ConcurrencyLimits {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let arc_mutext = &self.as_ref().get_ref().inner;

        let mut p = Poll::Ready(());
        if Arc::strong_count(arc_mutext) > 999 {
            p = Poll::Pending;

            let mut guard = arc_mutext.lock().unwrap();
            *guard = Some(cx.waker().clone());
        }
        p
    }
}

impl Drop for ConcurrencyLimits {
    fn drop(&mut self) {
        let arc_mutext = &self.inner;

        let mut guard = arc_mutext.lock().unwrap();
        if let Some(wk) = guard.take() {
            wk.wake();
        }
    }
}

async fn verify_one_conf(
    file_name: OsString,
    _concurrency_limit: ConcurrencyLimits,
) -> Option<OsString> {
    let f = File::open(file_name.clone()).await.unwrap();

    // Packing a layer of `BufReader` on top of `File` can satisfy AsyncBufRead -> AsyncBufReadExt.
    // then can call `lines`.

    let buf_reader = BufReader::new(f);
    let mut lines = buf_reader.lines();
    let mut state: i32 = 0;
    let mut through_file_name: Option<OsString> = None;
    let mut proto = Proto::Tcp;
    let mut address = String::new();
    let mut _addrees_buf = String::new();

    while let Some(line) = lines.next_line().await.unwrap_or_else(|e| {
        dbg!(e);
        None
    }) {
        if line.contains("proto") {
            state += 1;
            if line.trim_start_matches("proto").trim() == "udp" {
                proto = Proto::Udp;
            }
            // println!("{}", proto);
        }

        if line.contains("remote") {
            state += 1;
            let remote = line.trim_start_matches("remote").trim();
            //split

            //before
            // remote.split(" ").fold(&mut addrees_buf, |addres, e| {
            //     addres.push_str(e);
            //     addres.push_str(":");
            //     addres
            // });
            // address = addrees_buf.trim_end_matches(":");

            //after
            address = remote.splitn(2, " ").collect::<Vec<&str>>().join(":");
            // println!("{}", address);
        }

        if state == 2 {
            match proto {
                Proto::Udp => {
                    if verify_addres_by_udp(address).await {
                        through_file_name = Some(file_name);
                    }
                }

                Proto::Tcp => {
                    if verify_addres_by_tcp(address).await {
                        through_file_name = Some(file_name);
                    }
                }
            }

            return through_file_name;
        }
    }

    through_file_name

    // bind() the socket before you send your data. Specify port 0 to bind(),
    // and the OS will pick an unused port for you.
}

async fn verify_addres_by_udp(address: impl ToSocketAddrs + Debug) -> bool {
    // change to 0.0.0.0:0 is good.
    let sock = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let f = |e| println!("{}", e);

    sock.set_ttl(15).unwrap_or_else(f);

    // let remote_addr = address.parse::<SocketAddr>().unwrap();
    sock.connect(address)
        .await
        // .map_err(|e| dbg!(e))
        .map_err(|e| dbg!(e))
        .unwrap_or(());

    sock.send(b"hello world").await.unwrap_or(0);
    let mut buf = vec![0; 10];
    timeout(Duration::from_secs(15), sock.recv(&mut buf))
        .await
        .is_ok()

    //return true or false
}

async fn verify_addres_by_tcp(address: String) -> bool {
    // change to 0.0.0.0:0 is good.
    let sock = TcpSocket::new_v4().unwrap();

    // let remote_addr = address.parse::<SocketAddr>().unwrap();
    let stream: TcpStream = sock
        .connect(address.parse().unwrap())
        .await
        // .map_err(|e| dbg!(e))
        .map_err(|e| dbg!(e))
        .unwrap();

    timeout(Duration::from_secs(15), stream.writable())
        .await
        .is_ok()

    //return true or false
}

fn main() {
    let opt: Opt = Opt::from_args();
    let concurrency_limit = ConcurrencyLimits::default();
    let mut join_handle_vec = Vec::new();
    let mut process_num = 0;
    let mut success_num = 0;

    println!("{:#?}", opt);

    set_current_dir(opt.input.clone()).unwrap();
    let tr = Runtime::new().unwrap();
    tr.block_on(async {
        let mut entries = read_dir(opt.input).await.unwrap();

        while let Some(entry) = entries.next_entry().await.unwrap_or_else(|e| {
            println!("{}", e);
            None
        }) {
            let concurrency_limit_ref = concurrency_limit.clone();
            concurrency_limit_ref.await;
            let file_name = entry.file_name();

            // dbg!(file_name);
            join_handle_vec.push(spawn(verify_one_conf(file_name, concurrency_limit.clone())));
        }

        process_num = join_handle_vec.len();

        for j in join_handle_vec.into_iter() {
            if let Ok(o_s) = j.await {
                if let Some(s) = o_s {
                    if let Some(file_name) = s.into_string().ok() {
                        success_num += 1;
                        println!("{} : Connection successful", file_name);
                    }
                }
            }
        }
    });

    println!(
        "Total {}, successfully connected {}",
        process_num, success_num
    );
}
