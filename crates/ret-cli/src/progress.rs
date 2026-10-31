use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use ret_adb::download::{self, Progress};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct DownloadIndicator {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for DownloadIndicator {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn init() -> DownloadIndicator {
    start(download::subscribe())
}

fn start(receiver: Receiver<Progress>) -> DownloadIndicator {
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let worker = thread::Builder::new()
        .name("adb-progress".to_owned())
        .spawn(move || {
            let bar = ProgressBar::hidden();
            let known = ProgressStyle::with_template(
                "{spinner:.cyan} {msg} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} {percent}%",
            )
            .expect("valid download progress template");
            let unknown = ProgressStyle::with_template("{spinner:.cyan} {msg} {bytes}")
                .expect("valid download spinner template");
            let extracting = ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("valid extraction spinner template");
            let mut active = false;
            let mut update = |progress| match progress {
                Progress::Downloading { downloaded, total } => {
                    if !active {
                        bar.reset();
                        bar.set_draw_target(ProgressDrawTarget::stderr());
                        if !bar.is_hidden() {
                            bar.enable_steady_tick(Duration::from_millis(100));
                        }
                        active = true;
                    }
                    if let Some(total) = total.filter(|&total| total > 0) {
                        bar.set_style(known.clone());
                        bar.set_length(total);
                    } else {
                        bar.set_style(unknown.clone());
                        bar.unset_length();
                    }
                    bar.set_message("Downloading ADB tools");
                    bar.set_position(downloaded);
                }
                Progress::Extracting => {
                    bar.set_style(extracting.clone());
                    bar.set_message("Extracting ADB tools");
                }
                Progress::Failed(error) => {
                    bar.disable_steady_tick();
                    bar.set_style(extracting.clone());
                    bar.abandon_with_message(format!("ADB tools setup failed: {error}"));
                    active = false;
                }
                Progress::Ready | Progress::Idle => {
                    bar.disable_steady_tick();
                    bar.finish_and_clear();
                    active = false;
                }
            };
            while !stopped.load(Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(progress) => update(progress),
                    Err(RecvTimeoutError::Timeout) => (),
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            // A command can finish before the observer receives its final update.
            for progress in receiver.try_iter() {
                update(progress);
            }
            bar.disable_steady_tick();
            if !bar.is_finished() {
                bar.finish_and_clear();
            }
        })
        .expect("cannot spawn the ADB progress thread");
    DownloadIndicator {
        stop,
        worker: Some(worker),
    }
}
