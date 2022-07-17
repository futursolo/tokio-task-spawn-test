use std::time::{Duration, Instant};

use futures::channel::oneshot;
use futures::Future;
use tokio::spawn;
use tokio::task::{spawn_local, LocalSet};
use tokio_util::task::LocalPoolHandle;

mod rt;

pub use rt::Runtime;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

async fn run_test<Fut>(f: fn() -> Fut)
where
    Fut: 'static + Send + Future<Output = ()>,
{
    for round in 1..=5 {
        println!("  Round: {}", round);

        // spawn_local
        {
            let local_set = LocalSet::new();
            let start_time = Instant::now();

            local_set
                .run_until(async move {
                    let mut handles = Vec::with_capacity(1_000_000);

                    for _ in 0..1_000_000 {
                        let (tx, rx) = oneshot::channel();
                        spawn_local(async move {
                            f().await;
                            tx.send(()).expect("failed to send!")
                        });

                        handles.push(rx);
                    }

                    for handle in handles.into_iter() {
                        handle.await.expect("task failed.");
                    }
                })
                .await;

            println!(
                "        spawn local: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // spawn_pinned to local pool
        {
            let pool = LocalPoolHandle::new(num_cpus::get());
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                let (tx, rx) = oneshot::channel();
                pool.spawn_pinned(move || async move {
                    f().await;
                    tx.send(()).expect("failed to send!")
                });

                handles.push(rx);
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!(
                "       spawn pinned: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // spawn to runtime.
        {
            let runtime = Runtime::new();
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                let (tx, rx) = oneshot::channel();
                runtime.spawn(async move {
                    f().await;
                    tx.send(()).expect("failed to send!")
                });

                handles.push(rx);
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!(
                "         spawn (rt): {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // spawn_pinned to runtime.
        {
            let runtime = Runtime::new();
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                let (tx, rx) = oneshot::channel();
                runtime.spawn_pinned(move || async move {
                    f().await;
                    tx.send(()).expect("failed to send!")
                });

                handles.push(rx);
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!(
                "  spawn pinned (rt): {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // Spawn with default runtime.
        {
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                let (tx, rx) = oneshot::channel();
                spawn(async move {
                    f().await;
                    tx.send(()).expect("failed to send!")
                });

                handles.push(rx);
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!(
                "              spawn: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        println!();
    }
}

#[tokio::main]
async fn main() {
    println!("Testing Ready Future...");
    run_test(|| async {}).await;

    println!("Testing sleep(Duration::ZERO) Future...");
    run_test(|| tokio::time::sleep(Duration::ZERO)).await;
}
