use simplelog::{
    CombinedLogger, ConfigBuilder, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};
use std::env;
use std::fs::File;

pub mod test {
    //include!(concat!("", "load_balancer/test.rs"));
}

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
    /*let mut input_loop = InputLoop::new(String::from("127.0.0.1:50000"));
    input_loop.start();*/
}
