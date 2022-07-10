use std::time::{Duration, Instant};

use futures::stream::{FuturesOrdered, StreamExt};
use tokio_util::task::LocalPoolHandle;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

async fn task() {
    tokio::time::sleep(Duration::ZERO).await;
}

#[tokio::main]
async fn main() {
    let pool = LocalPoolHandle::new(num_cpus::get());

    for round in 1..=5 {
        println!("Round: {}", round);

        // Futures Ordered.
        {
            let start_time = Instant::now();
            let mut furs = FuturesOrdered::new();

            for _ in 0..1_000_000 {
                furs.push(task());
            }

            while furs.next().await.is_some() {}

            println!("  futures ordered: {}ms", start_time.elapsed().as_millis());
        }

        // spawn_local
        {
            let start_time = Instant::now();

            pool.spawn_pinned(|| async {
                let mut handles = Vec::with_capacity(1_000_000);

                for _ in 0..1_000_000 {
                    handles.push(tokio::task::spawn_local(task()));
                }

                for handle in handles.into_iter() {
                    handle.await.expect("task failed.");
                }
            })
            .await
            .expect("failed to complete.");

            println!("      spawn local: {}ms", start_time.elapsed().as_millis());
        }

        // spawn_pinned
        {
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                handles.push(pool.spawn_pinned(task));
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!("     spawn pinned: {}ms", start_time.elapsed().as_millis());
        }

        // Spawn with Send runtime.
        {
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                handles.push(tokio::task::spawn(task()));
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!("            spawn: {}ms", start_time.elapsed().as_millis());
        }

        println!();
    }
}
