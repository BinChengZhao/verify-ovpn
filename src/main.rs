use std::env::set_current_dir;
use std::net::SocketAddr;
use std::{ffi::OsString, path::PathBuf};
use structopt::StructOpt;
use tokio::fs::{read_dir, File};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;
use tokio::task::spawn;

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

async fn verify_one_conf(file_name: OsString) -> Option<OsString> {
    let f = File::open(file_name.clone()).await.unwrap();

    // Packing a layer of `BufReader` on top of `File` can satisfy AsyncBufRead -> AsyncBufReadExt.
    // then can call `lines`.

    let buf_reader = BufReader::new(f);
    let mut lines = buf_reader.lines();
    let mut state: i32 = 0;
    let mut through_file_name: Option<OsString> = None;
    let mut proto = Proto::Tcp;
    let mut address = "";
    let mut addrees_buf = String::new();

    while let Some(line) = lines.next_line().await.unwrap_or_else(|e| {
        println!("{}", e);
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
            remote.split(" ").fold(&mut addrees_buf, |addres, e| {
                addres.push_str(e);
                addres.push_str(":");
                addres
            });
            address = addrees_buf.trim_end_matches(":");
            // println!("{}", address);
        }

        if state == 2 {
            if proto == Proto::Udp {
                if verify_addres_by_udp(address).await {
                    through_file_name = Some(file_name);
                }

                return through_file_name;
            }
        }
    }

    through_file_name

    // bind() the socket before you send your data. Specify port 0 to bind(),
    // and the OS will pick an unused port for you.
}

async fn verify_addres_by_udp(address: &str) -> bool {
    let sock = UdpSocket::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let f = |e| println!("{}", e);

    sock.set_ttl(15).unwrap_or_else(f);

    let remote_addr = address.parse::<SocketAddr>().unwrap();
    sock.connect(dbg!(remote_addr))
        .await
        .map_err(|e| dbg!(e))
        .is_ok()

    //return true or false
}

fn main() {
    let opt: Opt = Opt::from_args();
    let mut join_handle_vec = Vec::new();
    let mut process_num = 0;

    println!("{:#?}", opt);

    set_current_dir(opt.input.clone()).unwrap();
    let tr = Runtime::new().unwrap();
    tr.block_on(async {
        let mut entries = read_dir(opt.input).await.unwrap();

        while let Some(entry) = entries.next_entry().await.unwrap_or_else(|e| {
            println!("{}", e);
            None
        }) {
            let file_name = entry.file_name();

            // dbg!(file_name);
            join_handle_vec.push(spawn(verify_one_conf(file_name)));
        }

        process_num = join_handle_vec.len();
        for j in join_handle_vec.into_iter() {
            if let Ok(o_s) = j.await {
                if let Some(s) = o_s {
                    if let Some(file_name) = s.into_string().ok() {
                        println!("{} : Connection successful", file_name);
                    }
                }
            }
        }
    });

    dbg!(process_num);
}
