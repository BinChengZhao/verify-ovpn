use std::env::set_current_dir;
use std::fmt::Debug;
use std::io::Result;
use std::path::PathBuf;

use structopt::StructOpt;

use tokio::fs::{read_dir, DirEntry, File};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;
use tokio::task::spawn;

use progress_bar::color::{Color, Style};
use progress_bar::progress_bar::ProgressBar;

pub(crate) mod verify;
use verify::*;

#[cfg(windows)]
const LINE_ENDING: &'static str = "\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &'static str = "\n";

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
    // Changing the program's working directory eliminates the need to specify an absolute path in front of each file.
    set_current_dir(opt.input.clone())?;

    let tr = Runtime::new()?;

    // when async block can't infer returen-type change it to async-fn so you can explict return-type.
    tr.block_on(verify_ovpns(opt.input, opt.output))
}

async fn verify_ovpns(input: PathBuf, output: PathBuf) -> Result<()> {
    // prepare data-source.
    let mut entry_buf = Vec::<DirEntry>::with_capacity(2048);
    let output_file = File::create(output).await?;

    let mut entries = read_dir(input).await?;

    while let Some(entry) = entries.next_entry().await? {
        entry_buf.push(entry);
    }

    // set Process bar.
    let mut progress_bar = ProgressBar::new(entry_buf.len() * 2);
    progress_bar.set_action("Loading", Color::Blue, Style::Bold);
    progress_bar.print_info("Hint", "The default is to check 1000 ovpn at a time concurrently.", Color::Green, Style::Bold);
   
    do_verify(entry_buf, output_file, progress_bar).await
}

async fn do_verify(
    entry_buf: Vec<DirEntry>,
    mut output_file: File,
    mut progress_bar: ProgressBar,
) -> Result<()> {
    let process_num;
    let mut success_num = 0;
    let concurrency_limit = ConcurrencyLimits::default();
    let mut join_handle_vec = Vec::new();

    for file_entry in entry_buf.into_iter() {
        progress_bar.inc();
        concurrency_limit.clone().await;

        let file_name = file_entry.file_name();
        join_handle_vec.push(spawn(verify_one_conf(file_name, concurrency_limit.clone())));
    }

    process_num = join_handle_vec.len();

    for j in join_handle_vec.into_iter() {
        progress_bar.inc();
        if let Ok(o_s) = j.await {
            if let Some(s) = o_s {
                if let Some(mut file_name) = s.into_string().ok() {
                    file_name += LINE_ENDING;

                    success_num += 1;
                    output_file.write(dbg!(file_name).as_bytes()).await?;
                }
            }
        }
    }

    output_file.sync_all().await?;

    let done_text = format!(
        "Total {}, successfully connected {}",
        process_num, success_num
    );

    progress_bar.print_final_info("Done", &done_text, Color::LightGreen, Style::Bold);

    Ok(())
}
