//! Wait-free hand-over of the engine between audio streams.
//!
//! When the output device changes, the old stream is dropped and a new one is built. The
//! engine (loaded tracks, positions, cue points) must survive that. Both streams hold an
//! [`EngineSlot`]; a callback borrows the engine with one atomic compare-exchange and never
//! waits. If the slot is busy (only possible during a hand-over), the callback outputs silence
//! for that block.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Engine;

struct Inner {
    busy: AtomicBool,
    engine: UnsafeCell<Engine>,
}

// SAFETY: access to `engine` is serialised by `busy` (see `try_with`); `Engine` itself is Send.
unsafe impl Sync for Inner {}

#[derive(Clone)]
pub struct EngineSlot {
    inner: Arc<Inner>,
}

/// Resets the busy flag even if the closure unwinds.
struct Release<'a>(&'a AtomicBool);

impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl EngineSlot {
    pub fn new(engine: Engine) -> EngineSlot {
        EngineSlot {
            inner: Arc::new(Inner {
                busy: AtomicBool::new(false),
                engine: UnsafeCell::new(engine),
            }),
        }
    }

    /// Run `f` with exclusive access to the engine, or return `None` immediately if someone
    /// else holds it. Never blocks, never allocates.
    #[inline]
    pub fn try_with<R>(&self, f: impl FnOnce(&mut Engine) -> R) -> Option<R> {
        if self
            .inner
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }
        let _release = Release(&self.inner.busy);
        // SAFETY: the successful compare-exchange above grants exclusive access until
        // `_release` drops; no other `try_with` can get past the flag in the meantime.
        let engine = unsafe { &mut *self.inner.engine.get() };
        Some(f(engine))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_pair;
    use rille_core::{Command, DeckCommand, DeckId};
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    #[test]
    fn never_two_users_at_once() {
        let (engine, _h) = engine_pair(48_000, 64);
        let slot = EngineSlot::new(engine);
        let inside = Arc::new(AtomicUsize::new(0));
        let entered = Arc::new(AtomicUsize::new(0));
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let slot = slot.clone();
                let inside = inside.clone();
                let entered = entered.clone();
                thread::spawn(move || {
                    for _ in 0..200_000 {
                        slot.try_with(|_| {
                            assert_eq!(inside.fetch_add(1, Ordering::SeqCst), 0, "overlap");
                            entered.fetch_add(1, Ordering::Relaxed);
                            inside.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        assert!(entered.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn state_survives_a_change_of_consumer() {
        let (engine, mut h) = engine_pair(48_000, 64);
        let slot = EngineSlot::new(engine);
        let first_stream = slot.clone();
        h.send(Command::Deck(DeckId::A, DeckCommand::Seek { frame: 0 }));
        first_stream.try_with(|e| e.note_xrun()).unwrap();
        drop(first_stream); // device gone
        let second_stream = slot.clone();
        second_stream
            .try_with(|e| e.process(&mut [0.0; 8], 2))
            .unwrap();
        assert_eq!(h.snapshot().xruns, 1, "same engine instance");
    }

    #[test]
    fn busy_slot_returns_none() {
        let (engine, _h) = engine_pair(48_000, 64);
        let slot = EngineSlot::new(engine);
        let nested = slot.try_with(|_| slot.try_with(|_| ()));
        assert_eq!(nested, Some(None));
        assert!(slot.try_with(|_| ()).is_some(), "released afterwards");
    }
}
