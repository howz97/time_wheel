use env_logger;
use hashbrown::HashMap;
use rand;
use std::thread::sleep;
use std::time::Duration;
use time_wheel::{unix_now_ms, FrontEnd};

const TW_INTER_MILLS: u64 = 32;
const WHEEL_SIZE: usize = 60;
const WHEEL_LVL: usize = 4;

#[test]
fn integer_timer() {
    env_logger::init();

    const TMR_COUNT: u32 = 200;
    const TMR_DELAY_RANG: u64 = 60000;

    let mut fe = FrontEnd::new(Duration::from_millis(TW_INTER_MILLS), WHEEL_SIZE, WHEEL_LVL);
    let mut timer_set = HashMap::new();

    // create TMR_COUNT timers
    for _ in 0..TMR_COUNT {
        let delay_mills = rand::random::<u64>() % TMR_DELAY_RANG;
        let tmr_id = fe.put_timer(Duration::from_millis(delay_mills));
        timer_set.insert(tmr_id, ());
    }

    // remove some timers
    for (id, _) in timer_set.drain_filter(|&id, _| id % 10 < 3) {
        fe.del_timer(id);
    }

    // all the left timers should trigger (check amount)
    for _ in 0..timer_set.len() {
        if let Ok(timer) = fe.receiver.recv() {
            if unix_now_ms() < timer.when {
                panic!("too earlier")
            }
            println!(
                " error={:?} timer trigger -> {:?}",
                unix_now_ms() - timer.when,
                timer
            );
            timer_set.remove(&timer.id).expect("timer loss");
        } else {
            panic!("receive error")
        }
    }
    assert_eq!(timer_set.len(), 0)
}

#[test]
fn integer_after_func() {
    let mut fe = FrontEnd::new(Duration::from_millis(TW_INTER_MILLS), WHEEL_SIZE, WHEEL_LVL);
    fe.after_func(Duration::from_millis(100), |t| {
        println!(
            " error={:?} func trigger -> {:?}",
            unix_now_ms() - t.when,
            t
        );
    });
    sleep(Duration::from_secs(1));
}
