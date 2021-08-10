use std::time::{Duration, SystemTime};
use time_wheel::FrontEnd;

fn main() {
    let mut fe = FrontEnd::new(Duration::from_millis(32), 60, 4);

    let time0 = SystemTime::now();
    let timer_id = fe.put_timer(Duration::from_millis(1500));
    println!("timer_id = {}", timer_id);
    let timer = fe.receiver.recv().unwrap();
    println!("Trigger {:?}", timer);
    println!("cost {:?}", time0.elapsed().unwrap());
}