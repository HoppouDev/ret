use std::fmt;
use std::sync::{Mutex, mpsc};

/// Current state of the bundled ADB tools installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Idle,
    Downloading { downloaded: u64, total: Option<u64> },
    Extracting,
    Ready,
    Failed(String),
}

impl fmt::Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => f.write_str("ADB tools have not been checked"),
            Self::Downloading { downloaded, total } => {
                if let Some(total) = total.filter(|total| *total > 0) {
                    let percent = (*downloaded as f64 / total as f64 * 100.0).min(100.0);
                    write!(
                        f,
                        "Downloading ADB tools: {percent:.0}% ({:.1} / {:.1} MiB)",
                        *downloaded as f64 / 1_048_576.0,
                        total as f64 / 1_048_576.0,
                    )
                } else if *downloaded == 0 {
                    f.write_str("Connecting to download ADB tools (0 bytes)")
                } else {
                    write!(f, "Downloading ADB tools: {downloaded} bytes")
                }
            }
            Self::Extracting => f.write_str("Extracting ADB tools"),
            Self::Ready => f.write_str("ADB tools are ready"),
            Self::Failed(error) => write!(f, "ADB tools installation failed: {error}"),
        }
    }
}

struct State {
    current: Progress,
    listeners: Vec<mpsc::Sender<Progress>>,
}

impl State {
    const fn new() -> Self {
        Self {
            current: Progress::Idle,
            listeners: Vec::new(),
        }
    }

    fn subscribe(&mut self) -> mpsc::Receiver<Progress> {
        let (sender, receiver) = mpsc::channel();
        let _ = sender.send(self.current.clone());
        self.listeners.push(sender);
        receiver
    }

    fn publish(&mut self, progress: Progress) {
        self.listeners
            .retain(|listener| listener.send(progress.clone()).is_ok());
        self.current = progress;
    }
}

static STATE: Mutex<State> = Mutex::new(State::new());

/// Observe the current installation state followed by subsequent updates.
///
/// Registration and the initial snapshot are atomic with respect to updates.
/// A failed installation remains visible until another attempt starts.
pub fn subscribe() -> mpsc::Receiver<Progress> {
    STATE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .subscribe()
}

pub(super) fn publish(progress: Progress) {
    STATE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .publish(progress);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_current_state_for_late_subscribers_across_retries() {
        let mut state = State::new();
        let early = state.subscribe();
        assert_eq!(early.recv().unwrap(), Progress::Idle);
        state.publish(Progress::Failed("offline".into()));
        assert_eq!(early.recv().unwrap(), Progress::Failed("offline".into()));
        let late = state.subscribe();
        assert_eq!(late.recv().unwrap(), Progress::Failed("offline".into()));
        drop(early);
        state.publish(Progress::Downloading {
            downloaded: 0,
            total: None,
        });
        assert_eq!(
            late.recv().unwrap(),
            Progress::Downloading {
                downloaded: 0,
                total: None
            }
        );
        state.publish(Progress::Ready);
        assert_eq!(late.recv().unwrap(), Progress::Ready);
        assert_eq!(state.subscribe().recv().unwrap(), Progress::Ready);
    }
}
