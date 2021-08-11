# timer
An implementation of timer uses time-wheel algorithm

---

1
```rust
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
```
output
```text
timer_id = 1
Trigger Timer(1, 1628603696.593087s)
cost 1.507511s
```

---

2
```rust
use std::time::{Duration};
use time_wheel::{FrontEnd, unix_now_ms};
use std::thread::sleep;

fn main() {
    let mut fe = FrontEnd::new(Duration::from_millis(32), 60, 4);
    fe.after_func(Duration::from_secs(1), |t| {
        println!(
            " error={:?} function is executing", unix_now_ms() - t.when);
    });
    sleep(Duration::from_secs(3));
}
```
output
```text
 error=27.973ms function is executing
```

---

3
```rust
use std::time::{Duration};
use time_wheel::{FrontEnd, unix_now_ms};

fn main() {
    let mut fe = FrontEnd::new(Duration::from_millis(32), 60, 4);
    let ticker = fe.ticker(Duration::from_millis(100));
    for i in 0..5 {
        let when = ticker.recv().unwrap();
        println!("on tick {} error={:?}", i, unix_now_ms() - when);
    }
}
```
output
```text
on tick 0 error=32.028ms
on tick 1 error=24.275ms
on tick 2 error=23.229ms
on tick 3 error=20.323ms
on tick 4 error=16.779ms
```