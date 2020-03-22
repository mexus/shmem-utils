use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::Rng;
use shmem::fixed_queue::FixedQueue;
use std::{
    iter::repeat_with,
    mem,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    thread,
    time::Duration,
};

const COUNT: usize = 1024;

#[derive(Clone)]
struct BigData([u64; COUNT]);

fn generate() -> Vec<BigData> {
    let mut rng = rand::thread_rng();
    (0..10_240)
        .map(|_| {
            let mut array = [0u64; COUNT];
            rng.fill(&mut array);
            BigData(array)
        })
        .collect()
}

fn looped(data: &[BigData]) -> impl Iterator<Item = BigData> + '_ {
    let mut cnt = 0;
    let len = data.len();
    repeat_with(move || {
        cnt = (cnt + 1) % len;
        data[cnt].clone()
    })
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let data = generate();
    let mut iter = looped(&data);

    let mut group = c.benchmark_group("shmem queue");
    group.throughput(Throughput::Bytes(mem::size_of::<BigData>() as u64));
    let requests = FixedQueue::<BigData>::new(1024).unwrap();
    let responds = FixedQueue::<BigData>::new(1024).unwrap();

    let running = Arc::new(AtomicBool::new(true));

    let server = {
        let requests = requests.clone();
        let responds = responds.clone();
        let running = Arc::clone(&running);
        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                if let Some(item) = requests.try_pop_front_timeout(Duration::from_secs(1)) {
                    responds.push_back(item);
                }
            }
        })
    };

    group.bench_function("ping", |b| {
        b.iter(|| {
            requests.push_back(iter.next().unwrap());
            if running.load(Ordering::Relaxed) {
                Some(responds.pop_back())
            } else {
                None
            }
        });
    });
    group.finish();
    running.store(false, Ordering::Relaxed);
    let _ = server.join();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);