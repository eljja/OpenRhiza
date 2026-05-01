use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    task::{Context, Poll, Waker},
};
use crossbeam_queue::ArrayQueue;
use alloc::task::Wake;

const DEFAULT_QUEUE_CAPACITY: usize = 256;
const DEFAULT_RUN_BATCH_BUDGET: usize = 64;

static SCHEDULER_WAKE_EVENTS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_WAKE_DROPS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_TOTAL_POLLS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_COMPLETED_TASKS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_BATCH_YIELDS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_IDLE_HALTS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_MAX_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_FORCE_RESCAN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub struct SchedulerMetricsSnapshot {
    pub wake_events: u64,
    pub wake_drops: u64,
    pub total_polls: u64,
    pub completed_tasks: u64,
    pub batch_yields: u64,
    pub idle_halts: u64,
    pub max_queue_depth: u64,
}

pub fn scheduler_metrics_snapshot() -> SchedulerMetricsSnapshot {
    SchedulerMetricsSnapshot {
        wake_events: SCHEDULER_WAKE_EVENTS.load(Ordering::Relaxed),
        wake_drops: SCHEDULER_WAKE_DROPS.load(Ordering::Relaxed),
        total_polls: SCHEDULER_TOTAL_POLLS.load(Ordering::Relaxed),
        completed_tasks: SCHEDULER_COMPLETED_TASKS.load(Ordering::Relaxed),
        batch_yields: SCHEDULER_BATCH_YIELDS.load(Ordering::Relaxed),
        idle_halts: SCHEDULER_IDLE_HALTS.load(Ordering::Relaxed),
        max_queue_depth: SCHEDULER_MAX_QUEUE_DEPTH.load(Ordering::Relaxed),
    }
}

fn note_queue_depth(depth: usize) {
    let depth = depth as u64;
    let mut current = SCHEDULER_MAX_QUEUE_DEPTH.load(Ordering::Relaxed);
    while depth > current {
        match SCHEDULER_MAX_QUEUE_DEPTH.compare_exchange(
            current,
            depth,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        SCHEDULER_WAKE_EVENTS.fetch_add(1, Ordering::Relaxed);
        if self.task_queue.push(self.task_id).is_err() {
            SCHEDULER_WAKE_DROPS.fetch_add(1, Ordering::Relaxed);
            SCHEDULER_FORCE_RESCAN.store(true, Ordering::Release);
        } else {
            note_queue_depth(self.task_queue.len());
        }
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
    run_batch_budget: usize,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(DEFAULT_QUEUE_CAPACITY)),
            waker_cache: BTreeMap::new(),
            run_batch_budget: DEFAULT_RUN_BATCH_BUDGET,
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
        note_queue_depth(self.task_queue.len());
    }

    fn run_ready_tasks(&mut self) -> usize {
        let mut processed = 0usize;
        while processed < self.run_batch_budget {
            let Some(task_id) = self.task_queue.pop() else {
                break;
            };
            if self.poll_task_once(task_id) {
                processed += 1;
            }
        }

        if processed < self.run_batch_budget && SCHEDULER_FORCE_RESCAN.swap(false, Ordering::AcqRel) {
            let task_ids: Vec<TaskId> = self.tasks.keys().copied().collect();
            for task_id in task_ids {
                if processed >= self.run_batch_budget {
                    SCHEDULER_FORCE_RESCAN.store(true, Ordering::Release);
                    break;
                }
                if self.poll_task_once(task_id) {
                    processed += 1;
                }
            }
        }
        processed
    }

    fn poll_task_once(&mut self, task_id: TaskId) -> bool {
        let Some(task) = self.tasks.get_mut(&task_id) else {
            return false;
        };
        let waker = self
            .waker_cache
            .entry(task_id)
            .or_insert_with(|| TaskWaker::new(task_id, self.task_queue.clone()));
        let mut context = Context::from_waker(waker);
        SCHEDULER_TOTAL_POLLS.fetch_add(1, Ordering::Relaxed);
        match task.poll(&mut context) {
            Poll::Ready(()) => {
                self.tasks.remove(&task_id);
                self.waker_cache.remove(&task_id);
                SCHEDULER_COMPLETED_TASKS.fetch_add(1, Ordering::Relaxed);
            }
            Poll::Pending => {}
        }
        true
    }

    pub fn run(&mut self) -> ! {
        loop {
            let processed = self.run_ready_tasks();
            if processed == 0 {
                self.sleep_if_idle();
                continue;
            }

            if processed >= self.run_batch_budget && !self.task_queue.is_empty() {
                SCHEDULER_BATCH_YIELDS.fetch_add(1, Ordering::Relaxed);
                x86_64::instructions::interrupts::enable();
                core::hint::spin_loop();
            }
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};
        interrupts::disable();
        if self.task_queue.is_empty() && !SCHEDULER_FORCE_RESCAN.load(Ordering::Acquire) {
            SCHEDULER_IDLE_HALTS.fetch_add(1, Ordering::Relaxed);
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}
