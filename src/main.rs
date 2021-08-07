use timer::{FrontEnd, unix_now_ms};
use std::time::{Duration};
use log::{trace};
use env_logger;
use rand;

fn main() {
    env_logger::init();
    let mut fe = FrontEnd::new(Duration::from_millis(8), 60, 3);
    for _ in 1..100000 {
        let delay_secs = rand::random::<u64>()%300;
        fe.put_timer(Duration::from_secs(delay_secs));
    }
    while let Ok(timer) = fe.receiver.recv() {
        if unix_now_ms() < timer.when {
            panic!("too earlier")
        }
        trace!(" error={:?} timer trigger -> {:?}", unix_now_ms() - timer.when, timer)
    }
}