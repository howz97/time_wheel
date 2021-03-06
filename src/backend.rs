use crossbeam::channel;
use crossbeam::channel::{RecvTimeoutError, TrySendError};
use hashbrown::HashMap;
use log::{info, trace, warn};
use std::fmt;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
struct Wheel {
    index: usize,
    slots: Vec<HashMap<usize, Timer>>,
    cur_slot: usize,
    next_ts: Duration, // is this necessary ？
    interval: Duration,
}

impl Wheel {
    fn new(index: usize, now: Duration, interval: Duration, size: usize) -> Self {
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
            for (_, t) in drained {
                triggered.push(t)
            }
            trace!("BackEnd wheel do forward: triggered timer {:?}", triggered);
        }
        triggered
    }
}

pub struct Timer {
    pub id: usize,
    pub when: Duration,

    opt_f: Option<Box<dyn FnOnce(Timer) + Send + 'static>>,

    period: Duration,
    sender: Option<channel::Sender<Duration>>,
}

impl Timer {
    pub fn normal(id: usize, when: Duration) -> Self {
        Timer {
            id,
            when,
            opt_f: None,
            period: Duration::new(0, 0),
            sender: None,
        }
    }

    pub fn after_func<F>(id: usize, when: Duration, f: F) -> Self
    where
        F: FnOnce(Timer) + Send + 'static,
    {
        Timer {
            id,
            when,
            opt_f: Some(Box::new(f)),
            period: Duration::new(0, 0),
            sender: None,
        }
    }

    pub fn ticker(id: usize, period: Duration, sender: channel::Sender<Duration>) -> Self {
        Timer {
            id,
            when: unix_now_ms() + period,
            opt_f: None,
            period,
            sender: Some(sender),
        }
    }
}

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Timer")
            .field(&self.id)
            .field(&self.when)
            .finish()
    }
}

pub enum Message {
    Put(Timer),
    Del(usize),
    Exit,
}

pub struct BackEnd {
    wheels: Vec<Wheel>,
    receiver: channel::Receiver<Message>,
    sender: channel::Sender<Timer>,
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
                    Message::Put(t) => self.put_timer(t),
                    Message::Del(id) => self.del_timer(id),
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
        let mut trigger = Vec::new();
        for (_, wheel) in self.wheels.iter_mut().enumerate().rev() {
            while let Some(t) = trigger.pop() {
                wheel
                    .put_timer(t)
                    .expect("put timer into lower wheel failed");
            }
            trigger = wheel.check_timer();
        }
        while let Some(t) = trigger.pop() {
            self.trigger(t);
        }
    }

    fn trigger(&mut self, mut timer: Timer) {
        trace!(
            "BackEnd.check_wheel sending {:?}, error={:?}",
            timer,
            unix_now_ms() - timer.when
        );
        if let Some(f) = timer.opt_f.take() {
            f(timer); // it should return instantly
        } else if let Some(sdr) = timer.sender.take() {
            if let Err(TrySendError::Disconnected(_)) = sdr.try_send(timer.when) {
                // drop ticker
            } else {
                timer.sender = Some(sdr);
                timer.when += timer.period;
                self.put_timer(timer);
            }
        } else {
            self.sender.send(timer).unwrap();
        }
    }

    fn put_timer(&mut self, timer: Timer) {
        let mut put_result = Err(timer);
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

pub fn unix_now_ms() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}
