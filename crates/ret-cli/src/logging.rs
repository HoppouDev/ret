use std::io;
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Log errors to stderr and the rest to stdout, filtered by `RUST_LOG` (`info` by default)
pub fn init() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_owned()))
        .with_writer(io::stderr.with_max_level(Level::ERROR).or_else(io::stdout))
        .init();
}
