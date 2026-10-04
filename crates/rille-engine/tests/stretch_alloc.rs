#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code
//! Signalsmith Stretch is C++: its allocations bypass Rust's global allocator, so
//! `assert_no_alloc` cannot see them. macOS's `malloc_logger` hook reports every malloc/new of
//! the process; we count those made by this thread while the stretcher processes.

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    type MallocLogger = unsafe extern "C" fn(u32, usize, usize, usize, usize, u32);
    unsafe extern "C" {
        static mut malloc_logger: Option<MallocLogger>;
        fn pthread_self() -> usize;
    }

    static WATCHED_THREAD: AtomicUsize = AtomicUsize::new(0);
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    pub static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
    const ALLOCATE: u32 = 2;

    unsafe extern "C" fn count(kind: u32, _: usize, _: usize, _: usize, _: usize, _: u32) {
        if kind & ALLOCATE != 0
            && ACTIVE.load(Ordering::Relaxed)
            && unsafe { pthread_self() } == WATCHED_THREAD.load(Ordering::Relaxed)
        {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Count this thread's heap allocations (Rust and C/C++) while `f` runs.
    pub fn allocations_during(f: impl FnOnce()) -> usize {
        WATCHED_THREAD.store(unsafe { pthread_self() }, Ordering::Relaxed);
        ALLOCATIONS.store(0, Ordering::Relaxed);
        unsafe { malloc_logger = Some(count) };
        ACTIVE.store(true, Ordering::Relaxed);
        f();
        ACTIVE.store(false, Ordering::Relaxed);
        unsafe { malloc_logger = None };
        ALLOCATIONS.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "macos")]
#[test]
fn stretch_process_never_allocates() {
    use signalsmith_stretch::Stretch;
    let sr = 48_000u32;
    let mut s = Stretch::preset_default(2, sr);
    let block = 256usize;
    let mut out = vec![0.0f32; block * 2];
    let mut phase = 0u64;
    // Music-like input: several tones plus noise, so many spectral peaks appear.
    let mut input = |n: usize| -> Vec<f32> {
        (0..n)
            .flat_map(|_| {
                phase += 1;
                let t = phase as f32 / sr as f32;
                let noise = ((phase.wrapping_mul(6364136223846793005) >> 33) as f32
                    / (1u64 << 31) as f32)
                    - 0.5;
                let v = 0.2 * (t * 110.0 * std::f32::consts::TAU).sin()
                    + 0.2 * (t * 440.0 * std::f32::consts::TAU).sin()
                    + 0.1 * (t * 3520.0 * std::f32::consts::TAU).sin()
                    + 0.2 * noise;
                [v, v]
            })
            .collect()
    };
    // Counter-check: the probe sees a plain allocation.
    let seen = macos::allocations_during(|| {
        std::hint::black_box(vec![0u8; 64]);
    });
    assert!(seen >= 1, "malloc_logger probe does not work");

    // Keylock at tempo -16 % … +16 %: input length per output block varies accordingly.
    let lengths = [215usize, 230, 256, 270, 297, 256, 241];
    let inputs: Vec<Vec<f32>> = (0..4000)
        .map(|i| input(lengths[i % lengths.len()]))
        .collect();
    let mut total = 0;
    for (i, chunk) in inputs.iter().enumerate() {
        total += macos::allocations_during(|| {
            if i % 500 == 499 {
                // A cue jump with keylock: start over and pre-roll from the new position.
                s.reset();
                s.seek(chunk, 1.0);
            }
            s.process(chunk, &mut out)
        });
    }
    assert_eq!(total, 0, "Signalsmith Stretch allocated during process()");
}
