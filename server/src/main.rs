use simplelog::{ConfigBuilder, CombinedLogger, TermLogger, TerminalMode, WriteLogger, LevelFilter};
use std::fs::File;

fn main() {
    let mut config = ConfigBuilder::new();
    config.set_location_level(LevelFilter::Error);
    config.set_thread_level(LevelFilter::Error);
    config.set_time_level(LevelFilter::Error);
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Trace, config.build(), TerminalMode::Stdout),
            WriteLogger::new(LevelFilter::Error, config.build(), File::create("Logs/master.log").unwrap()),
        ]
    ).unwrap();
}
