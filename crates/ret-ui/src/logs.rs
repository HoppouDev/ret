//! Logs printed to the terminal and shown in the window
//!
//! The window must stay responsive however long it runs and however much is logged: lines reach
//! it in batches, the oldest ones are dropped in blocks, and each kept line has a stable id so
//! that the lines already on screen are not laid out again when new ones arrive.

use anyhow::Context as _;
use iced::Subscription;
use iced::futures::{Stream, stream};
use std::collections::VecDeque;
use std::fmt::{self, Write};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber, info, warn};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{self, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, registry};

/// Log files kept, one per day
const LOG_FILES: usize = 7;

/// Lines waiting for a window that does not read them fast enough, older ones are skipped
const CHANNEL_CAPACITY: usize = 4096;

/// Lines kept in the window
const MAX_LINES: usize = 1000;

/// Lines dropped at once when [`MAX_LINES`] is exceeded
///
/// Dropping one line per new line would shift every row, so that each one is laid out again.
const DROPPED_LINES: usize = 250;

/// Rows of text the window shows at most, a line taking one row or more when it wraps
///
/// After the first drop, at least `MAX_LINES - DROPPED_LINES` lines are always kept, each at
/// least one row: shown unfiltered, their rows always fill this many, so that the height of the
/// panel content stays the same from then on, whatever lines are dropped. A level filter shows
/// fewer lines, whose height changes as old ones are dropped: padding them to this height would
/// leave a long blank area to scroll through above a few errors.
pub const MAX_ROWS: usize = MAX_LINES - DROPPED_LINES;

/// Delay to gather the lines logged together into one update of the window
const BATCH_DELAY: Duration = Duration::from_millis(100);

/// Most lines delivered in one update, so that a flood cannot delay the window
const MAX_BATCH: usize = 512;

static LINES: LazyLock<broadcast::Sender<Line>> =
    LazyLock::new(|| broadcast::channel(CHANNEL_CAPACITY).0);

/// One logged event
#[derive(Debug, Clone)]
pub struct Line {
    pub level: Level,
    /// The component which logged it, such as `TcpConnection`
    pub target: String,
    pub message: String,
}

/// The default log filter: the logs of ret, and only the problems of the libraries drawing the
/// window, which are verbose
const DEFAULT_FILTER: &str = "info,iced=warn,iced_winit=warn,iced_wgpu=warn,iced_tiny_skia=warn,\
                              wgpu=warn,wgpu_core=warn,wgpu_hal=error,naga=warn,winit=warn";

/// Print the logs to the terminal, save them to a file of [`log_dir`] and send them to the window,
/// filtered by `RUST_LOG` ([`DEFAULT_FILTER`] by default)
///
/// The file is written in the background: the returned guard writes the last lines when dropped,
/// so it must live until the window closes.
pub fn init() -> Option<WorkerGuard> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let file = open_log_file();
    let file_layer = file.as_ref().ok().map(|(writer, _, _)| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(writer.clone())
    });
    registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(file_layer)
        .with(WindowLayer)
        .init();
    match file {
        Ok((_, guard, dir)) => {
            info!("Logs saved in {}", dir.display());
            Some(guard)
        }
        Err(err) => {
            warn!("{err:#}, the logs are not saved");
            None
        }
    }
}

/// Where the log files are saved: `<data_dir>/ret/logs`
fn log_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("ret").join("logs"))
}

/// A writer to the log file of the day, its guard, and the directory of the files
fn open_log_file() -> anyhow::Result<(NonBlocking, WorkerGuard, PathBuf)> {
    let dir = log_dir().context("no data directory")?;
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let appender = rolling::Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("ret-ui")
        .filename_suffix("log")
        .max_log_files(LOG_FILES)
        .build(&dir)
        .with_context(|| format!("cannot write the logs to {}", dir.display()))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    Ok((writer, guard, dir))
}

/// Sends every event to the window, as structured data rather than formatted text
struct WindowLayer;

impl<S: Subscriber> Layer<S> for WindowLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        if LINES.receiver_count() == 0 {
            // no window listening yet, do not format anything
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        // events forwarded from the `log` crate carry their real target in a field
        let target = visitor
            .log_target
            .unwrap_or_else(|| event.metadata().target().to_owned());
        // no window listening is not an error
        let _ = LINES.send(Line {
            level: *event.metadata().level(),
            target,
            message: single_line(visitor.message),
        });
    }
}

/// `message` as one paragraph, which the window wraps to its width: each line break becomes a
/// space, without the indentation after it, so that pretty-printed values stay compact
fn single_line(message: String) -> String {
    if !message.contains('\n') {
        return message;
    }
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Formats the fields of an event: the message, then the other fields as `key=value`
#[derive(Default)]
struct MessageVisitor {
    message: String,
    log_target: Option<String>,
}

impl MessageVisitor {
    fn record(&mut self, field: &Field, value: fmt::Arguments<'_>) {
        match field.name() {
            "message" => self.message.insert_str(0, &value.to_string()),
            "log.target" => self.log_target = Some(value.to_string()),
            // where a `log` event was emitted, noise in the window
            "log.module_path" | "log.file" | "log.line" => (),
            name => {
                let _ = write!(self.message, " {name}={value}");
            }
        }
    }
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, format_args!("{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record(field, format_args!("{value:?}"));
    }
}

/// The lines logged from the moment the subscription starts, gathered in batches
pub fn subscription() -> Subscription<Vec<Line>> {
    Subscription::run(batches)
}

fn batches() -> impl Stream<Item = Vec<Line>> {
    stream::unfold(LINES.subscribe(), |mut receiver| async move {
        let mut batch = Vec::new();
        // wait for the first line, then for the others logged meanwhile
        match receiver.recv().await {
            Ok(line) => batch.push(line),
            Err(RecvError::Lagged(skipped)) => batch.push(skipped_line(skipped)),
            Err(RecvError::Closed) => return None,
        }
        tokio::time::sleep(BATCH_DELAY).await;
        while batch.len() < MAX_BATCH {
            match receiver.try_recv() {
                Ok(line) => batch.push(line),
                Err(TryRecvError::Lagged(skipped)) => batch.push(skipped_line(skipped)),
                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
            }
        }
        Some((batch, receiver))
    })
}

/// Reports the lines logged faster than the window could show them
fn skipped_line(skipped: u64) -> Line {
    Line {
        level: Level::WARN,
        target: "ret_ui".to_owned(),
        message: format!("{skipped} lines skipped, logged faster than the window shows them"),
    }
}

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
    /// The kept lines, oldest first, each with an id that never changes
    lines: VecDeque<(u64, Line)>,
    next_id: u64,
    errors: usize,
    warnings: usize,
    pub visible: bool,
    pub filter: LogFilter,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            next_id: 0,
            errors: 0,
            warnings: 0,
            // collapsed until asked for, the devices matter more
            visible: false,
            filter: LogFilter::All,
        }
    }
}

impl Logs {
    pub fn extend(&mut self, lines: impl IntoIterator<Item = Line>) {
        for line in lines {
            match line.level {
                Level::ERROR => self.errors += 1,
                Level::WARN => self.warnings += 1,
                _ => (),
            }
            self.lines.push_back((self.next_id, line));
            self.next_id += 1;
        }
        if self.lines.len() > MAX_LINES {
            let excess = self.lines.len() - MAX_LINES;
            let dropped = excess.div_ceil(DROPPED_LINES) * DROPPED_LINES;
            for (_, line) in self.lines.drain(..dropped.min(self.lines.len())) {
                match line.level {
                    Level::ERROR => self.errors -= 1,
                    Level::WARN => self.warnings -= 1,
                    _ => (),
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.errors = 0;
        self.warnings = 0;
    }

    /// The lines passing the filter with their ids, oldest first
    pub fn shown(&self) -> impl Iterator<Item = (u64, &Line)> {
        self.lines
            .iter()
            .filter(|(_, line)| self.filter.shows(line.level))
            .map(|(id, line)| (*id, line))
    }

    /// Number of errors kept
    pub fn errors(&self) -> usize {
        self.errors
    }

    /// Number of warnings kept
    pub fn warnings(&self) -> usize {
        self.warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(level: Level, message: &str) -> Line {
        Line {
            level,
            target: "test".to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn keep_a_bounded_number_of_lines() {
        let mut logs = Logs::default();
        for i in 0..10 * MAX_LINES {
            logs.extend([line(Level::INFO, &i.to_string())]);
            assert!(logs.lines.len() <= MAX_LINES);
        }
        // the newest lines are kept
        let last = logs.shown().last().unwrap().1;
        assert_eq!((10 * MAX_LINES - 1).to_string(), last.message);
    }

    #[test]
    fn keep_enough_lines_to_fill_the_shown_rows() {
        // the panel height stays fixed only if the kept lines always fill MAX_ROWS rows, each line
        // taking at least one
        let mut logs = Logs::default();
        for batch in 1..=40 {
            logs.extend((0..batch * 7).map(|i| line(Level::INFO, &i.to_string())));
            if logs.next_id > MAX_LINES as u64 {
                assert!(logs.lines.len() >= MAX_ROWS, "{} lines", logs.lines.len());
            }
        }
    }

    #[test]
    fn drop_old_lines_in_blocks() {
        let mut logs = Logs::default();
        logs.extend((0..MAX_LINES).map(|i| line(Level::INFO, &i.to_string())));
        let first_id = logs.shown().next().unwrap().0;

        // the lines on screen keep their ids until a whole block is dropped
        logs.extend([line(Level::INFO, "one more")]);
        assert_eq!(MAX_LINES + 1 - DROPPED_LINES, logs.lines.len());
        assert_eq!(
            first_id + DROPPED_LINES as u64,
            logs.shown().next().unwrap().0
        );
        for _ in 1..DROPPED_LINES {
            logs.extend([line(Level::INFO, "more")]);
        }
        assert_eq!(MAX_LINES, logs.lines.len());
        assert_eq!(
            first_id + DROPPED_LINES as u64,
            logs.shown().next().unwrap().0
        );
    }

    #[test]
    fn count_the_kept_problems() {
        let mut logs = Logs::default();
        logs.extend([line(Level::ERROR, "e"), line(Level::WARN, "w")]);
        logs.extend((0..MAX_LINES).map(|_| line(Level::INFO, "i")));
        // the problems were the oldest lines, they are dropped with their block
        assert_eq!((0, 0), (logs.errors(), logs.warnings()));

        logs.extend([line(Level::WARN, "w"), line(Level::WARN, "w")]);
        assert_eq!((0, 2), (logs.errors(), logs.warnings()));
        logs.clear();
        assert_eq!((0, 0), (logs.errors(), logs.warnings()));
    }

    #[test]
    fn show_multiline_messages_on_one_line() {
        let pretty = "Available adapters: [\n    AdapterInfo {\n        vendor: 4098,\n    },\n]";
        assert_eq!(
            "Available adapters: [ AdapterInfo { vendor: 4098, }, ]",
            single_line(pretty.to_owned())
        );
        // blank lines and indentation leave a single space
        assert_eq!(
            "blank lines go",
            single_line("blank lines\n\n\tgo".to_owned())
        );
        assert_eq!("as it is", single_line("as it is".to_owned()));
    }

    #[test]
    fn filter_by_level() {
        let mut logs = Logs::default();
        logs.extend([
            line(Level::DEBUG, "d"),
            line(Level::INFO, "i"),
            line(Level::WARN, "w"),
            line(Level::ERROR, "e"),
        ]);
        let shown = |logs: &Logs| -> Vec<String> {
            logs.shown().map(|(_, line)| line.message.clone()).collect()
        };
        assert_eq!(vec!["d", "i", "w", "e"], shown(&logs));
        logs.filter = LogFilter::Warnings;
        assert_eq!(vec!["w", "e"], shown(&logs));
        logs.filter = LogFilter::Errors;
        assert_eq!(vec!["e"], shown(&logs));
    }
}
