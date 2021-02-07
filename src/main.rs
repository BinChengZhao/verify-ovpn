use std::path::PathBuf;
use structopt::StructOpt;
use tokio::fs::{read_dir, File};
use tokio::io::{AsyncBufReadExt, BufReader};
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

fn main() {
    let opt: Opt = Opt::from_args();

    println!("{:#?}", opt);

    let tr = Runtime::new().unwrap();
    tr.block_on(async {
        let mut entries = read_dir(opt.input).await.unwrap();

        while let Some(entry) = entries.next_entry().await.unwrap_or_else(|e| {
            println!("{}", e);
            None
        }) {
            let file_name = entry.file_name();

            spawn(async move {
                let f = File::open(file_name).await.unwrap();

                // Packing a layer of `BufReader` on top of `File` can satisfy AsyncBufRead -> AsyncBufReadExt.
                // then can call `lines`.
                let buf_reader = BufReader::new(f);
                let mut lines = buf_reader.lines();

                while let Some(line) = lines.next_line().await.unwrap_or_else(|e| {
                    println!("{}", e);
                    None
                }) {
                    println!("length = {}", line.len())
                }
            });
        }
    });
}
