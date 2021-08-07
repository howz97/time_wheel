use timer::{FrontEnd, unix_now_ms};
use std::time::{Duration};
use log::{info};
use env_logger;

fn main() {
    env_logger::init();
    let mut fe = FrontEnd::new(Duration::from_millis(8), 60, 3);
    for delay in 1..10000 {
        fe.put_timer(Duration::from_secs(delay));
    }
    while let timer = fe.rcv_timer() {
        if unix_now_ms() < timer.when {
            panic!("too earlier")
        }
        info!(" error={:?} timer trigger -> {:?}", unix_now_ms() - timer.when, timer)
    }
}