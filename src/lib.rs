mod backend;

pub use backend::{unix_now_ms, Timer};
use backend::{BackEnd, Message};
use crossbeam::channel;
use std::thread;
use std::time::Duration;

pub struct FrontEnd {
    unused_id: usize,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: channel::Sender<Message>,
    pub receiver: channel::Receiver<Timer>,
}

impl FrontEnd {
    pub fn new(interval: Duration, size: usize, lv: usize) -> Self {
        let (op_sdr, op_rcv) = channel::bounded(0);
        let (tmr_sdr, tmr_rcv) = channel::unbounded();
        FrontEnd {
            unused_id: 1,
            join_handle: Some(BackEnd::new(interval, size, lv, op_rcv, tmr_sdr)),
            sender: op_sdr,
            receiver: tmr_rcv,
        }
    }

    // Put a timer into TimeWheel by specify delay, return the timer_id
    pub fn put_timer(&mut self, delay: Duration) -> usize {
        let id = self.unused_id;
        let timer = Timer::normal(id, unix_now_ms() + delay);
        self.sender.send(Message::Put(timer)).unwrap();
        self.unused_id += 1;
        id
    }

    // Delete timer
    pub fn del_timer(&mut self, timer_id: usize) {
        self.sender.send(Message::Del(timer_id)).unwrap();
    }

    pub fn after_func<F>(&mut self, delay: Duration, f: F)
    where
        F: FnOnce(Timer) + Send + 'static,
    {
        let timer = Timer::after_func(self.unused_id, unix_now_ms() + delay, f);
        self.sender.send(Message::Put(timer)).unwrap();
        self.unused_id += 1;
    }

    pub fn ticker(&mut self, period: Duration) -> channel::Receiver<Duration> {
        let (sdr, rcv) = channel::bounded(1);
        let timer = Timer::ticker(self.unused_id, period, sdr);
        self.sender.send(Message::Put(timer)).unwrap();
        self.unused_id += 1;
        rcv
    }
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
