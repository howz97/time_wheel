use crossbeam::channel;
use crossbeam::channel::RecvTimeoutError;
use hashbrown::HashMap;
use log::{info, trace, warn};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct FrontEnd {
    unused_id: usize,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: channel::Sender<Message>,
    pub receiver: channel::Receiver<Timer>,
}

impl FrontEnd {
    pub fn new(interval: Duration, size: usize, lv: usize) -> FrontEnd {
        let (op_sdr, op_rcv) = channel::bounded(0);
        let (tmr_sdr, tmr_rcv) = channel::unbounded();
        FrontEnd {
            unused_id: 1,
            join_handle: Some(BackEnd::new(interval, size, lv, op_rcv, tmr_sdr)),
            sender: op_sdr,
            receiver: tmr_rcv,
        }
    }

    // Put a timer into TimeWheel by specify trigger_after, return the timer_id
    pub fn put_timer(&mut self, delay: Duration) -> usize {
        let timer_id = self.unused_id;
        self.unused_id += 1;
        let when = unix_now_ms() + delay;
        self.sender.send(Message::Put(timer_id, when)).unwrap();
        timer_id
    }

    // Delete timer
    pub fn del_timer(&mut self, timer_id: usize) {
        self.sender.send(Message::Del(timer_id)).unwrap();
    }
}

pub fn unix_now_ms() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}

impl Drop for FrontEnd {
    fn drop(&mut self) {
        self.sender.send(Message::Exit).unwrap();
        self.join_handle
            .take()
            .unwrap()
            .join()
            .expect("time_wheel: backend thread panicked");
    }
}

enum Message {
    Put(usize, Duration),
    Del(usize),
    Exit,
}

struct BackEnd {
    wheels: Vec<Wheel>,
    receiver: channel::Receiver<Message>,
    sender: channel::Sender<Timer>,
}

#[derive(Debug)]
struct Wheel {
    index: usize,
    slots: Vec<HashMap<usize, Timer>>,
    cur_slot: usize,
    next_ts: Duration, // is this necessary ？
    interval: Duration,
}

impl Wheel {
    fn new(index: usize, now: Duration, interval: Duration, size: usize) -> Wheel {
        let mut slots = Vec::with_capacity(size);
        for _ in 0..size {
            slots.push(HashMap::new())
        }
        Wheel {
            index,
            slots,
            cur_slot: 0,
            next_ts: now + interval,
            interval,
        }
    }

    fn try_forward(&mut self) -> bool {
        if self.next_ts < unix_now_ms() {
            self.cur_slot += 1;
            self.cur_slot %= self.slots.len();
            self.next_ts += self.interval;
            true
        } else {
            false
        }
    }

    fn cur_ts(&self) -> Duration {
        self.next_ts - self.interval
    }

    fn cur_slot_map(&mut self) -> &mut HashMap<usize, Timer> {
        &mut self.slots[self.cur_slot]
    }

    fn put_timer(&mut self, timer: Timer) -> Result<(), Timer> {
        if let Ok(pos) = self.calc_timer_pos(timer.when) {
            trace!(
                "BackEnd.put_timer {:?} into wheel({})-slot({})",
                timer,
                self.index,
                pos
            );
            self.slots[pos].insert(timer.id, timer);
            Ok(())
        } else {
            Err(timer)
        }
    }

    fn calc_timer_pos(&self, when: Duration) -> Result<usize, ()> {
        let cur_ts = self.cur_ts();
        if when <= cur_ts {
            return Ok(1);
        }
        let pos = (((when - cur_ts).as_nanos() / self.interval.as_nanos()) + 1) as usize;
        if pos > self.slots.len() {
            return Err(());
        }
        Ok((self.cur_slot + pos) % self.slots.len())
    }

    fn check_timer(&mut self) -> Vec<Timer> {
        let mut triggered = Vec::new();
        while self.try_forward() {
            let cur_ts = self.cur_ts();
            trace!(
                "BackEnd wheel do forward: cur_ts={:?} now={:?}",
                cur_ts,
                self
            );
            let drained: HashMap<usize, Timer> = self
                .cur_slot_map()
                .drain_filter(|_, t| t.when < cur_ts)
                .collect();
            let tmp = drained.values().cloned().collect::<Vec<_>>();
            triggered = [triggered, tmp].concat();
            trace!("BackEnd wheel do forward: triggered timer {:?}", triggered);
        }
        triggered
    }
}

#[derive(Debug)]
pub struct Timer {
    pub id: usize,
    pub when: Duration,
}

impl Timer {
    fn new(id: usize, when: Duration) -> Timer {
        Timer { id, when }
    }
}

impl Clone for Timer {
    fn clone(&self) -> Timer {
        Timer { ..*self }
    }
}

impl BackEnd {
    pub fn new(
        mut interval: Duration,
        wheel_size: usize,
        wheel_lv: usize,
        rcv: channel::Receiver<Message>,
        sdr: channel::Sender<Timer>,
    ) -> thread::JoinHandle<()> {
        assert_ne!(wheel_lv, 0);
        assert_ne!(wheel_size, 0);

        let mut wheels = Vec::new();
        let now = unix_now_ms();
        info!("BackEnd initialed start ts = {:?}", now);
        for i in 0..wheel_lv {
            wheels.push(Wheel::new(i, now, interval, wheel_size));
            interval *= wheel_size as u32;
        }
        let back_end = BackEnd {
            wheels,
            receiver: rcv,
            sender: sdr,
        };
        thread::spawn(move || back_end.run())
    }

    fn run(mut self) {
        loop {
            match self.receiver.recv_timeout(self.calc_wait_timeout()) {
                Ok(op) => match op {
                    Message::Put(timer_id, when) => self.put_timer(timer_id, when),
                    Message::Del(timer_id) => self.del_timer(timer_id),
                    Message::Exit => {
                        info!("BackEnd received Exit");
                        break;
                    }
                },
                Err(RecvTimeoutError::Disconnected) => {
                    warn!("BackEnd received Disconnected");
                    break;
                }
                _ => trace!("BackEnd timeout: need check timers now={:?}", unix_now_ms()),
            }
            self.check_wheel();
        }
    }

    fn calc_wait_timeout(&self) -> Duration {
        let now = unix_now_ms();
        if self.wheels[0].next_ts <= now {
            Duration::new(0, 0)
        } else {
            self.wheels[0].next_ts - now
        }
    }

    fn check_wheel(&mut self) {
        trace!("BackEnd.check_wheel");
        let mut triggered = Vec::new();
        for (_, wheel) in self.wheels.iter_mut().enumerate().rev() {
            while let Some(t) = triggered.pop() {
                wheel
                    .put_timer(t)
                    .expect("put timer into lower wheel failed");
            }
            triggered = wheel.check_timer();
        }
        while let Some(t) = triggered.pop() {
            trace!(
                "BackEnd.check_wheel sending {:?}, error={:?}",
                t,
                unix_now_ms() - t.when
            );
            self.sender.send(t).unwrap();
        }
    }

    fn put_timer(&mut self, id: usize, when: Duration) {
        let mut put_result = Err(Timer::new(id, when));
        // try wheel from low to high
        for wheel in self.wheels.iter_mut() {
            if let Err(timer) = put_result {
                put_result = wheel.put_timer(timer);
            } else {
                break;
            }
        }
        if let Err(timer) = put_result {
            // insert timer that overflowed into tail slot
            warn!("insert timer={:?} that overflowed into tail slot", timer);
            self.wheels
                .last_mut()
                .unwrap()
                .cur_slot_map()
                .insert(timer.id, timer);
        }
    }

    fn del_timer(&mut self, id: usize) {
        trace!("BackEnd.del_timer id={}", id);
        for wheel in self.wheels.iter_mut() {
            for slot in wheel.slots.iter_mut() {
                if let Some(_) = slot.remove(&id) {
                    return;
                }
            }
        }
    }
}
