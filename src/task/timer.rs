use alloc::vec::Vec;
use core::{
    pin::Pin,
    task::{Context, Poll, Waker},
};
use spin::Mutex;

pub const TICKS_PER_SECOND: u64 = 1_000;
pub static TIMER_WAKERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());
pub static TICKS: Mutex<u64> = Mutex::new(0);

pub const fn ms_to_ticks(ms: u64) -> u64 {
    let ticks = (ms * TICKS_PER_SECOND + (TICKS_PER_SECOND - 1)) / TICKS_PER_SECOND;
    if ticks == 0 { 1 } else { ticks }
}

pub fn timer_tick() {
    let mut ticks = TICKS.lock();
    *ticks += 1;
    for waker in TIMER_WAKERS.lock().drain(..) {
        waker.wake();
    }
}

pub struct TimerFuture {
    target_tick: u64,
}

impl TimerFuture {
    pub fn new(delay_ticks: u64) -> Self {
        let current = *TICKS.lock();
        TimerFuture { target_tick: current + delay_ticks }
    }
}

impl core::future::Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let current = *TICKS.lock();
        if current >= self.target_tick {
            Poll::Ready(())
        } else {
            let mut wakers = TIMER_WAKERS.lock();
            if !wakers.iter().any(|waker| waker.will_wake(cx.waker())) {
                wakers.push(cx.waker().clone());
            }
            Poll::Pending
        }
    }
}

pub async fn sleep_ticks(ticks: u64) {
    TimerFuture::new(ticks).await;
}
