use std::time::{Duration, Instant};

use futures::stream::{FuturesOrdered, StreamExt};
use futures::Future;
use tokio_util::task::LocalPoolHandle;

async fn run_test<Fut>(f: fn() -> Fut)
where
    Fut: 'static + Send + Future<Output = ()>,
{
    let pool = LocalPoolHandle::new(num_cpus::get());

    for round in 1..=5 {
        println!("  Round: {}", round);

        // Futures Ordered.
        {
            let start_time = Instant::now();
            let mut furs = FuturesOrdered::new();

            for _ in 0..1_000_000 {
                furs.push(f());
            }

            while furs.next().await.is_some() {}

            println!(
                "    futures ordered: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // spawn_local
        {
            let start_time = Instant::now();

            pool.spawn_pinned(move || async move {
                let mut handles = Vec::with_capacity(1_000_000);

                for _ in 0..1_000_000 {
                    handles.push(tokio::task::spawn_local(f()));
                }

                for handle in handles.into_iter() {
                    handle.await.expect("task failed.");
                }
            })
            .await
            .expect("failed to complete.");

            println!(
                "        spawn local: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // spawn_pinned to local pool
        {
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                handles.push(pool.spawn_pinned(f));
            }

            for handle in handles.into_iter() {
                handle.await.expect("task failed.");
            }

            println!(
                "       spawn pinned: {}ms",
                start_time.elapsed().as_millis()
            );
        }

        // Spawn with default runtime.
        {
            let start_time = Instant::now();
            let mut handles = Vec::with_capacity(1_000_000);

            for _ in 0..1_000_000 {
                handles.push(tokio::task::spawn(f()));
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
