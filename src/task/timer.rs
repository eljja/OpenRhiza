use core::{pin::Pin, task::{Poll, Context, Waker}};
use spin::Mutex;

pub static TIMER_WAKER: Mutex<Option<Waker>> = Mutex::new(None);
pub static TICKS: Mutex<u64> = Mutex::new(0);

pub fn timer_tick() {
    let mut ticks = TICKS.lock();
    *ticks += 1;
    if let Some(waker) = TIMER_WAKER.lock().take() {
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
            *TIMER_WAKER.lock() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub async fn sleep_ticks(ticks: u64) {
    TimerFuture::new(ticks).await;
}
