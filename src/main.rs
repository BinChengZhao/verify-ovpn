use std::env::set_current_dir;
use std::fmt::Debug;
use std::io::Result;
use std::path::PathBuf;

use structopt::StructOpt;

use tokio::fs::{read_dir, File};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;
use tokio::task::spawn;

pub(crate) mod verify;
use verify::*;

/// Structual of verify-ovpn.
#[derive(StructOpt, Debug)]
#[structopt(name = "verify-ovpn")]
struct Opt {
    /// Input path (Absolute path)
    #[structopt(short, long, parse(from_os_str))]
    input: PathBuf,

    /// Output path (Absolute path)
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let concurrency_limit = ConcurrencyLimits::default();

    println!("{:#?}", opt);

    // Changing the program's working directory eliminates the need to specify an absolute path in front of each file.
    set_current_dir(opt.input.clone())?;

    let tr = Runtime::new()?;

    // when async block can't infer returen-type change it to async-fn so you can explict return-type.
    tr.block_on(verify_ovpns(opt.input, opt.output, concurrency_limit))
}

async fn verify_ovpns(
    input: PathBuf,
    output: PathBuf,
    concurrency_limit: ConcurrencyLimits,
) -> Result<()> {
    let mut output_file = File::create(output).await?;
    let mut join_handle_vec = Vec::new();
    let mut success_num = 0;
    let process_num;

    let mut entries = read_dir(input).await?;

    while let Some(entry) = entries.next_entry().await? {
        concurrency_limit.clone().await;

        let file_name = entry.file_name();
        join_handle_vec.push(spawn(verify_one_conf(file_name, concurrency_limit.clone())));
    }

    process_num = join_handle_vec.len();

    for j in join_handle_vec.into_iter() {
        if let Ok(o_s) = j.await {
            if let Some(s) = o_s {
                if let Some(file_name) = s.into_string().ok() {
                    success_num += 1;
                    // TODO: Add EOL.
                    output_file.write(file_name.as_bytes()).await?;
                }
            }
        }
    }

    output_file.sync_all().await?;

    println!(
        "Total {}, successfully connected {}",
        process_num, success_num
    );

    Ok(())
}
