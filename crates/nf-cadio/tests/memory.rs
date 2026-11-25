//! Peak heap use of the parser, measured by a counting allocator, against the size of the input.
//! This binary has one test, so no other test allocates while a measurement runs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use nf_cadio::{Document, Reason};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(n: usize) {
    let now = LIVE.fetch_add(n, Ordering::SeqCst) + n;
    PEAK.fetch_max(now, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
            grow(new_size);
        }
        p
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Runs `f` and returns its result with the most bytes allocated above the starting level at any point.
fn peak_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = LIVE.load(Ordering::SeqCst);
    PEAK.store(base, Ordering::SeqCst);
    let out = f();
    (out, PEAK.load(Ordering::SeqCst).saturating_sub(base))
}

/// An OBJECTS section of `pairs` pairs, each `5` and a one-byte value: four bytes a pair, the worst
/// ratio of memory to input.
fn worst(pairs: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(pairs * 4 + 64);
    v.extend_from_slice(b"  0\nSECTION\n  2\nOBJECTS\n");
    for _ in 0..pairs {
        v.extend_from_slice(b"5\n\xff\n");
    }
    v.extend_from_slice(b"  0\nENDSEC\n  0\nEOF\n");
    v
}

#[test]
fn parser_memory_is_bounded_by_the_input_and_the_pair_limit() {
    // 1 MiB of worst-case pairs is accepted. Its peak heap use must stay within a fixed multiple of the
    // input, which is the bound that the pair limit makes true for any accepted input.
    let input = worst(262_144);
    let (doc, peak) = peak_during(|| Document::parse(&input).expect("accepted"));
    eprintln!("memory: input {} bytes, peak {} bytes", input.len(), peak);
    assert!(
        peak <= 16 * input.len() + (64 << 10),
        "peak {peak} for input {}",
        input.len()
    );
    drop(doc);

    // 5 million pairs (20 MB) under the default limits are refused at pair 4,000,001. Nothing is
    // stored before the refusal, so the peak is far below the input.
    let big = worst(5_000_000);
    let (result, peak) = peak_during(|| Document::parse(&big));
    let e = result.expect_err("over the pair limit");
    assert!(matches!(e.reason, Reason::TooManyPairs { .. }), "{e:?}");
    eprintln!(
        "memory: refused input {} bytes, peak {} bytes",
        big.len(),
        peak
    );
    assert!(peak < 1 << 20, "peak {peak} for a refused input");
}
