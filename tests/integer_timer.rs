use timer::{FrontEnd};
use std::time::{Duration};
use std::thread::sleep;

#[test]
fn integer_timer() {
    let mut fe = FrontEnd::new(Duration::from_secs(1), 3, 3);
    for _ in 1..3 {
        fe.put_timer(Duration::from_secs(1));
    }
    let timer = fe.rcv_timer();
    println!("timer trigger -> {:?}", timer)
    // while let timer = fe.rcv_timer() {
    //     println!("timer trigger -> {:?}", timer)
    // }
}