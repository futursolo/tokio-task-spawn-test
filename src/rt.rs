use futures::channel::mpsc::UnboundedSender;
use futures::stream::StreamExt;

use std::future::Future;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime as BaseRuntime};
use tokio::task::{spawn_local, LocalSet};

type SpawnTask = Box<dyn Send + FnOnce()>;

struct Worker {
    task_count: Arc<AtomicUsize>,
    tx: UnboundedSender<SpawnTask>,
}

impl Worker {
    fn new(rt_inner: Arc<BaseRuntime>) -> Self {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<SpawnTask>();

        let task_count: Arc<AtomicUsize> = Arc::default();

        std::thread::Builder::new()
            .name("yew-runtime-worker".into())
            .spawn(move || {
                let local_set = LocalSet::new();

                rt_inner.block_on(local_set.run_until(async move {
                    while let Some(m) = rx.next().await {
                        m();
                    }
                }));
            })
            .expect("failed to spawn worker thread.");

        Self { task_count, tx }
    }
}

pub struct JobCountGuard(Arc<AtomicUsize>);

impl JobCountGuard {
    fn new(inner: Arc<AtomicUsize>) -> Self {
        inner.fetch_add(1, Ordering::Relaxed);
        JobCountGuard(inner)
    }
}

impl Drop for JobCountGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct Runtime {
    inner: Arc<BaseRuntime>,
    workers: Arc<Vec<Worker>>,
}

impl Runtime {
    pub fn new() -> Self {
        let rt = Builder::new_multi_thread()
            // We need to have at least 1 tokio spawned worker thread for a mutil threaded runtime.
            .worker_threads(1)
            .thread_name("yew-runtime-worker")
            .enable_all()
            .build()
            .expect("failed to create runtime.");

        let rt = Arc::new(rt);

        let workers = (0..num_cpus::get())
            .map(|_| Worker::new(rt.clone()))
            .collect::<Vec<_>>();

        Self {
            inner: rt,
            workers: workers.into(),
        }
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.inner.spawn(f);
    }

    // fn find_random_worker(&self) -> &Worker {
    //     use rand::prelude::SliceRandom;
    //     use rand::thread_rng;

    //     let mut rng = thread_rng();
    //     self.workers
    //         .choose(&mut rng)
    //         .expect("must have more than 1 worker.")
    // }

    fn find_least_busy_worker(&self) -> &Worker {
        let mut workers = self.workers.iter();

        let mut worker = workers.next().expect("must have more than 1 worker.");
        let mut task_count = worker.task_count.load(Ordering::Relaxed);

        for current_worker in workers.take(3) {
            if task_count == 0 {
                // Use a for loop here so we don't have to search until the end.
                break;
            }

            let current_worker_task_count = current_worker.task_count.load(Ordering::Relaxed);

            if current_worker_task_count < task_count {
                task_count = current_worker_task_count;
                worker = current_worker;
            }
        }

        worker
    }

    pub fn spawn_pinned<F, Fur>(&self, f: F)
    where
        F: FnOnce() -> Fur + Send + 'static,
        Fur: Future<Output = ()> + 'static,
    {
        let worker = self.find_least_busy_worker();
        let task_count = worker.task_count.clone();
        let guard = JobCountGuard::new(task_count);

        let _ = worker.tx.unbounded_send(Box::new(move || {
            spawn_local(async move {
                let _guard = guard;

                f().await;
            });
        }));
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // We must drop workers first, let all local sets quit,
        // then drop the Runtime in a different thread.
        // Dropping a runtime is a blocking behaviour and will panic in async context.
        let inner = self.inner.clone();

        // Creating a new thread may fail if the main thread is unwinding.
        // It is safe to drop in current thread in that case.
        let _ = std::thread::Builder::new()
            .name("yew-runtime-unwinding-thread".into())
            .spawn(move || {
                let mut inner = inner;
                loop {
                    // Can only unwrap if only 1 strong count exists.
                    // Which means all other worker threads have exited.
                    match Arc::try_unwrap(inner) {
                        Ok(_) => {
                            // Drop value.
                            break;
                        }
                        Err(m) => {
                            std::thread::sleep(Duration::from_millis(1));
                            inner = m;
                        }
                    }
                }
            });
    }
}
