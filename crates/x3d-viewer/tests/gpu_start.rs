//! The shared GPU start (`common::gpu`): parallel callers start one at a time, each caller gets its
//! own device, each device works after the start, and the helper keeps each device. Without an
//! adapter (hosted CI) the devices are `None`, and only the ordering checks run.

mod common;

use common::gpu;
use std::collections::HashSet;
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

const CALLERS: usize = 8;

/// A copy and a read back on `device`, to show that it works after the start.
fn works(device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.clear_buffer(&buffer, 0, None);
    queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r.is_ok()).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).is_ok() && rx.try_recv() == Ok(true)
}

#[test]
fn parallel_gpu_starts_run_one_at_a_time_with_own_devices() {
    let (before, kept_before) = (gpu::starts(), gpu::kept());
    let barrier = Arc::new(Barrier::new(CALLERS));
    let callers: Vec<_> = (0..CALLERS)
        .map(|_| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                gpu::device()
            })
        })
        .collect();
    let devices: Vec<_> = callers.into_iter().map(|c| c.join().unwrap()).collect();

    assert!(gpu::starts() - before >= CALLERS);
    assert_eq!(gpu::peak(), 1, "two GPU starts ran at the same time");

    let devices: Vec<_> = devices.into_iter().flatten().collect();

    if devices.is_empty() {
        return eprintln!("no adapter, device checks skipped");
    }

    assert_eq!(
        devices.len(),
        CALLERS,
        "a caller got no device while others did"
    );
    let distinct: HashSet<_> = devices.iter().map(|(d, _)| d.clone()).collect();
    assert_eq!(distinct.len(), CALLERS, "two callers share one device");

    for (d, q) in &devices {
        assert!(works(d, q), "a device does not work after the start");
    }

    assert!(
        gpu::kept() - kept_before >= CALLERS,
        "the helper does not keep each device"
    );
}

#[test]
fn a_held_device_does_not_block_the_next_start() {
    let first = gpu::device();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || tx.send(gpu::device()).unwrap());
    let second = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the second start waited for the first device");

    if let (Some((d1, q1)), Some((d2, q2))) = (&first, &second) {
        assert_ne!(d1, d2, "the second start returned the first device");
        assert!(works(d1, q1) && works(d2, q2));
    }

    assert_eq!(gpu::peak(), 1, "two GPU starts ran at the same time");
}

#[test]
fn renderer_probes_and_device_starts_share_the_lock() {
    let barrier = Arc::new(Barrier::new(4));
    let callers: Vec<_> = (0..4)
        .map(|i| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if i % 2 == 0 {
                    gpu::renderer_available()
                } else {
                    gpu::device().is_some()
                }
            })
        })
        .collect();
    let found: Vec<_> = callers.into_iter().map(|c| c.join().unwrap()).collect();

    assert_eq!(gpu::peak(), 1, "two GPU starts ran at the same time");
    assert!(
        found.iter().all(|&f| f == found[0]),
        "the probe and the device start disagree on the adapter"
    );
}
