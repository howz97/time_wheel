# timer
An implementation of timer uses time-wheel algorithm

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

fn integer_after_func() {
    let mut fe = FrontEnd::new(Duration::from_millis(32), 60, 4);
    fe.after_func(Duration::from_millis(100), |t| {
        println!(
            " error={:?} func trigger -> {:?}",
            unix_now_ms() - t.when,
            t
        );
    });
    sleep(Duration::from_secs(1));
}
```
output
```text
 error=32.338ms func trigger -> Timer(1, 1628646155.685307s)
```