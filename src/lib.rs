use crossbeam::channel;
use crossbeam::channel::{select};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FrontEnd {
    unused_id: usize,
    join_handle: Option<thread::JoinHandle<()>>,
    sender: channel::Sender<Message>,
    receiver: channel::Receiver<Timer>,
}

impl FrontEnd {
    pub fn new() -> FrontEnd {
        let (op_sdr, op_rcv) = channel::bounded(0);
        let (tmr_sdr, tmr_rcv) = channel::unbounded();
        FrontEnd {
            unused_id: 1,
            join_handle: Some(BackEnd::new(op_rcv, tmr_sdr)),
            sender: op_sdr,
            receiver: tmr_rcv,
        }
    }

    // Put a timer into TimeWheel by specify trigger_after, return the timer_id
    pub fn put_timer(&mut self, trigger_after: u128) -> usize {
        let timer_id = self.unused_id;
        self.unused_id += 1;
        let trigger_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + trigger_after;
        self.sender.send(Message::Put(timer_id, trigger_at)).unwrap();
        timer_id
    }

    // Delete timer
    pub fn del_timer(&mut self, timer_id: usize) {
        self.sender.send(Message::Del(timer_id)).unwrap();
    }

    // Receive a trigger timer
    pub fn rcv_timer(&mut self) -> Timer {
        self.receiver.recv().unwrap()
    }

    // Try receive a trigger timer
    pub fn try_rcv_timer(&mut self) -> Option<Timer> {
        if let Ok(tmr) = self.receiver.try_recv() {
            Some(tmr)
        } else {
            None
        }
    }
}

impl Drop for FrontEnd {
    fn drop(&mut self) {
        self.sender.send(Message::Exit).unwrap();
        self.join_handle.take().unwrap().join().expect("time_wheel: backend thread panicked");
    }
}

enum Message {
    Put(usize, u128),
    Del(usize),
    Exit,
}

struct BackEnd {
    receiver: channel::Receiver<Message>,
    sender: channel::Sender<Timer>,
}

pub struct Timer {
    pub id: usize,
    pub trigger_ts: usize,
}

impl BackEnd {
    pub fn new(
        rcv: channel::Receiver<Message>,
        sdr: channel::Sender<Timer>,
    ) -> thread::JoinHandle<()> {
        let back_end = BackEnd {
            receiver: rcv,
            sender: sdr,
        };
        thread::spawn(move || back_end.run())
    }

    fn run(self) {
        select! {
            recv(self)
            default
        }
    }
}
