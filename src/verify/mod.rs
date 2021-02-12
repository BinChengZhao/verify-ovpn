use std::ffi::OsString;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

mod tcp;
mod udp;

use tcp::verify_addres_by_tcp;
use udp::verify_addres_by_udp;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Proto {
    Udp,
    Tcp,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConcurrencyLimits {
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

pub(crate) async fn verify_one_conf(
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
        }

        if state == 2 {
            match proto {
                Proto::Udp => {
                    if verify_addres_by_udp(address).await.is_ok() {
                        through_file_name = Some(file_name);
                    }
                }

                Proto::Tcp => {
                    if verify_addres_by_tcp(address).await.is_ok() {
                        through_file_name = Some(file_name);
                    }
                }
            }

            return through_file_name;
        }
    }

    through_file_name
}
