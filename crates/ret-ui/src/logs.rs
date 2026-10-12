//! Logs printed to the terminal and shown in the window

use iced::Subscription;
use iced::futures::{Stream, stream};
use std::collections::VecDeque;
use std::io;
use std::sync::LazyLock;
use tokio::sync::broadcast;
use tracing::{Level, Metadata};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Lines kept for a window that does not read them fast enough
const CAPACITY: usize = 1024;

static LINES: LazyLock<broadcast::Sender<Line>> = LazyLock::new(|| broadcast::channel(CAPACITY).0);

#[derive(Debug, Clone)]
pub struct Line {
    pub level: Level,
    pub text: String,
}

/// Lines kept in the window
const MAX_LINES: usize = 500;

/// Which lines the window shows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFilter {
    All,
    Warnings,
    Errors,
}

impl LogFilter {
    fn shows(self, level: Level) -> bool {
        match self {
            LogFilter::All => true,
            LogFilter::Warnings => matches!(level, Level::WARN | Level::ERROR),
            LogFilter::Errors => level == Level::ERROR,
        }
    }
}

/// The recent lines and how the window shows them
pub struct Logs {
    lines: VecDeque<Line>,
    pub visible: bool,
    pub filter: LogFilter,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            visible: true,
            filter: LogFilter::All,
        }
    }
}

impl Logs {
    pub fn push(&mut self, line: Line) {
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// The lines passing the filter, oldest first
    pub fn shown(&self) -> impl Iterator<Item = &Line> {
        self.lines
            .iter()
            .filter(|line| self.filter.shows(line.level))
    }

    /// Number of lines kept at `level`
    pub fn count(&self, level: Level) -> usize {
        self.lines.iter().filter(|line| line.level == level).count()
    }
}

/// The default log filter: the logs of ret, and only the problems of the libraries drawing the
/// window, which are verbose
const DEFAULT_FILTER: &str = "info,iced=warn,iced_winit=warn,iced_wgpu=warn,iced_tiny_skia=warn,\
                              wgpu=warn,wgpu_core=warn,wgpu_hal=error,naga=warn,winit=warn";

/// Print the logs to the terminal and send them to the window, filtered by `RUST_LOG`
/// ([`DEFAULT_FILTER`] by default)
pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_timer(ChronoLocal::new("%H:%M:%S".to_owned()))
                .with_writer(MakeLineWriter),
        )
        .init();
}

/// The log lines, from the moment the subscription starts
pub fn subscription() -> Subscription<Line> {
    Subscription::run(lines)
}

fn lines() -> impl Stream<Item = Line> {
    let receiver = LINES.subscribe();
    stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(line) => return Some((line, receiver)),
                // the oldest lines were dropped, keep the recent ones
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
}

struct MakeLineWriter;

impl<'a> MakeWriter<'a> for MakeLineWriter {
    type Writer = LineWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LineWriter::new(Level::INFO)
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        LineWriter::new(*meta.level())
    }
}

/// Collects one formatted event, sent as a line once complete
struct LineWriter {
    level: Level,
    buf: Vec<u8>,
}

impl LineWriter {
    fn new(level: Level) -> Self {
        Self {
            level,
            buf: Vec::new(),
        }
    }
}

impl io::Write for LineWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for LineWriter {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.buf).trim_end().to_owned();
        if !text.is_empty() {
            // no window listening is not an error
            let _ = LINES.send(Line {
                level: self.level,
                text,
            });
        }
    }
}
