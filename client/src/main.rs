use std::fs::File;

use simplelog::{
    CombinedLogger, ConfigBuilder, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};

use client::InputLoop;

fn main() {
    let mut config = ConfigBuilder::new();
    config.set_location_level(LevelFilter::Error);
    config.set_thread_level(LevelFilter::Error);
    config.set_time_level(LevelFilter::Error);
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Trace, config.build(), TerminalMode::Stdout),
        WriteLogger::new(
            LevelFilter::Error,
            config.build(),
            File::create("Logs/master.log").unwrap(),
        ),
    ])
    .unwrap();
    let mut input = InputLoop::new(String::from("127.0.0.1:5000"));
    input.start();
}
