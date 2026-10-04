//! Default function assignment: [`ControlEvent`] → [`ControllerAction`].
//!
//! The assignment is one `match` over (control, shift, value) — the table of the spec — and the
//! only state it keeps is whether each jog is touched (scratch vs. bend).

use rille_core::DeckId;

use super::decode::{ControlEvent, ControlValue};
use super::table::{Control, Scope};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqBand {
    Hi,
    Mid,
    Low,
}

/// What the user meant, independent of which bytes the controller sent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControllerAction {
    PlayPause(DeckId),
    /// Held for as long as shift + play is held.
    Reverse {
        deck: DeckId,
        on: bool,
    },
    Cue {
        deck: DeckId,
        pressed: bool,
    },
    JumpToStart(DeckId),
    SyncOnce(DeckId),
    SyncLockToggle(DeckId),
    CycleTempoRange(DeckId),
    /// Tempo fader position, -1.0 (top, "−", slowest) ..= +1.0 (bottom, "+", fastest).
    Tempo {
        deck: DeckId,
        value: f32,
    },
    PitchBend {
        deck: DeckId,
        ticks: i32,
    },
    ScratchTouch {
        deck: DeckId,
        touching: bool,
    },
    Scratch {
        deck: DeckId,
        ticks: i32,
    },
    Search {
        deck: DeckId,
        ticks: i32,
    },
    LibraryScroll(i32),
    LoadSelected(DeckId),
    HeadphoneCue(DeckId),
    Eq {
        deck: DeckId,
        band: EqBand,
        value: f32,
    },
    /// Colour FX knob: 0.0 low-pass … 0.5 neutral … 1.0 high-pass.
    Filter {
        deck: DeckId,
        value: f32,
    },
    ChannelFader {
        deck: DeckId,
        value: f32,
    },
    Crossfader(f32),
    FaderStart {
        deck: DeckId,
        play: bool,
    },
    MasterCue,
    TransitionFx {
        pressed: bool,
    },
    CycleTransitionFx,
    Pad {
        deck: DeckId,
        index: u8,
        pressed: bool,
        shifted: bool,
    },
    ShiftHeld {
        deck: DeckId,
        held: bool,
    },
}

#[derive(Debug, Default)]
pub struct Mapper {
    touching: [bool; 2],
}

/// Tempo fader 0..1 (0 = top) → -1..+1 with the top meaning slower, as printed on the device.
pub fn tempo_from_fader(position: f32) -> f32 {
    (position.clamp(0.0, 1.0) * 2.0) - 1.0
}

impl Mapper {
    pub fn new() -> Mapper {
        Mapper::default()
    }

    pub fn reset(&mut self) {
        self.touching = [false; 2];
    }

    pub fn map(&mut self, e: ControlEvent) -> Option<ControllerAction> {
        use ControlValue::{Absolute, Button, Relative};
        use ControllerAction as A;

        let Scope::Deck(deck) = e.scope else {
            return match (e.control, e.shifted, e.value) {
                (Control::Crossfader, _, Absolute(v)) => Some(A::Crossfader(v)),
                (Control::MasterCue, false, Button(true)) => Some(A::MasterCue),
                (Control::TransitionFx, false, Button(p)) => Some(A::TransitionFx { pressed: p }),
                (Control::TransitionFx, true, Button(true)) => Some(A::CycleTransitionFx),
                _ => None,
            };
        };

        match (e.control, e.shifted, e.value) {
            (Control::Shift, _, Button(held)) => Some(A::ShiftHeld { deck, held }),
            (Control::Play, false, Button(true)) => Some(A::PlayPause(deck)),
            (Control::Play, true, Button(on)) => Some(A::Reverse { deck, on }),
            (Control::Cue, false, Button(pressed)) => Some(A::Cue { deck, pressed }),
            (Control::Cue, true, Button(true)) => Some(A::JumpToStart(deck)),
            (Control::Sync, false, Button(true)) => Some(A::SyncOnce(deck)),
            (Control::SyncLong, _, Button(true)) => Some(A::SyncLockToggle(deck)),
            (Control::Sync, true, Button(true)) => Some(A::CycleTempoRange(deck)),
            (Control::TempoFader, _, Absolute(v)) => Some(A::Tempo {
                deck,
                value: tempo_from_fader(v),
            }),
            (Control::JogTouch, false, Button(touching)) => {
                self.touching[deck.index()] = touching;
                Some(A::ScratchTouch { deck, touching })
            }
            (Control::JogTouch, true, Button(false)) => {
                // Shift released the platter: make sure a scratch does not hang.
                let was = std::mem::take(&mut self.touching[deck.index()]);
                was.then_some(A::ScratchTouch {
                    deck,
                    touching: false,
                })
            }
            (Control::JogPlatterVinyl, _, Relative(ticks)) if self.touching[deck.index()] => {
                Some(A::Scratch { deck, ticks })
            }
            (Control::JogPlatterVinyl | Control::JogPlatter, _, Relative(ticks)) => {
                Some(A::PitchBend { deck, ticks })
            }
            (Control::JogRim, false, Relative(ticks)) => Some(A::PitchBend { deck, ticks }),
            (Control::JogRim, true, Relative(ticks)) => Some(A::LibraryScroll(ticks)),
            (Control::JogPlatterShift, _, Relative(ticks)) => Some(A::Search { deck, ticks }),
            (Control::EqHi, _, Absolute(v)) => Some(A::Eq {
                deck,
                band: EqBand::Hi,
                value: v,
            }),
            (Control::EqMid, _, Absolute(v)) => Some(A::Eq {
                deck,
                band: EqBand::Mid,
                value: v,
            }),
            (Control::EqLow, _, Absolute(v)) => Some(A::Eq {
                deck,
                band: EqBand::Low,
                value: v,
            }),
            (Control::ColorFx, _, Absolute(v)) => Some(A::Filter { deck, value: v }),
            (Control::ChannelFader, _, Absolute(v)) => Some(A::ChannelFader { deck, value: v }),
            (Control::FaderStartPlay, _, Button(true)) => Some(A::FaderStart { deck, play: true }),
            (Control::FaderStartCue, _, Button(true)) => Some(A::FaderStart { deck, play: false }),
            (Control::HeadphoneCue, false, Button(true)) => Some(A::HeadphoneCue(deck)),
            (Control::HeadphoneCue, true, Button(true)) => Some(A::LoadSelected(deck)),
            (Control::Pad(index), shifted, Button(pressed)) => Some(A::Pad {
                deck,
                index,
                pressed,
                shifted,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::decode::Decoder;
    use super::*;
    use ControllerAction as A;
    use DeckId::{A as DA, B as DB};

    /// Feed raw bytes through decoder and mapper, collect actions.
    fn run(msgs: &[&[u8]]) -> Vec<ControllerAction> {
        let mut d = Decoder::new();
        let mut m = Mapper::new();
        msgs.iter()
            .filter_map(|b| d.decode(b).and_then(|e| m.map(e)))
            .collect()
    }

    #[test]
    fn play_and_reverse() {
        assert_eq!(
            run(&[&[0x90, 0x0B, 0x7F], &[0x90, 0x0B, 0x00]]),
            [A::PlayPause(DA)]
        );
        assert_eq!(
            run(&[&[0x91, 0x47, 0x7F], &[0x91, 0x47, 0x00]]),
            [
                A::Reverse { deck: DB, on: true },
                A::Reverse {
                    deck: DB,
                    on: false
                }
            ]
        );
    }

    #[test]
    fn cue_press_release_and_shift_cue() {
        assert_eq!(
            run(&[&[0x90, 0x0C, 0x7F], &[0x90, 0x0C, 0x00]]),
            [
                A::Cue {
                    deck: DA,
                    pressed: true
                },
                A::Cue {
                    deck: DA,
                    pressed: false
                }
            ]
        );
        assert_eq!(
            run(&[&[0x90, 0x48, 0x7F], &[0x90, 0x48, 0x00]]),
            [A::JumpToStart(DA)]
        );
    }

    #[test]
    fn sync_variants() {
        assert_eq!(run(&[&[0x90, 0x58, 0x7F]]), [A::SyncOnce(DA)]);
        assert_eq!(run(&[&[0x90, 0x5C, 0x7F]]), [A::SyncLockToggle(DA)]);
        assert_eq!(run(&[&[0x91, 0x60, 0x7F]]), [A::CycleTempoRange(DB)]);
    }

    #[test]
    fn tempo_top_is_slower() {
        assert_eq!(tempo_from_fader(0.0), -1.0, "top / minus side");
        assert_eq!(tempo_from_fader(1.0), 1.0, "bottom / plus side");
        assert_eq!(tempo_from_fader(0.5), 0.0);
        assert_eq!(
            run(&[&[0xB0, 0x00, 0x00], &[0xB0, 0x20, 0x00]]),
            [A::Tempo {
                deck: DA,
                value: -1.0
            }]
        );
    }

    #[test]
    fn jog_touch_turns_platter_into_scratch() {
        let actions = run(&[
            &[0xB0, 0x22, 0x42], // platter, not touched → bend
            &[0x90, 0x36, 0x7F], // touch
            &[0xB0, 0x22, 0x3E], // platter touched → scratch
            &[0x90, 0x36, 0x00], // release
            &[0xB0, 0x22, 0x41], // bend again
        ]);
        assert_eq!(
            actions,
            [
                A::PitchBend { deck: DA, ticks: 2 },
                A::ScratchTouch {
                    deck: DA,
                    touching: true
                },
                A::Scratch {
                    deck: DA,
                    ticks: -2
                },
                A::ScratchTouch {
                    deck: DA,
                    touching: false
                },
                A::PitchBend { deck: DA, ticks: 1 },
            ]
        );
    }

    #[test]
    fn vinyl_off_platter_bends() {
        assert_eq!(
            run(&[&[0x90, 0x36, 0x7F], &[0xB0, 0x23, 0x45]])[1],
            A::PitchBend { deck: DA, ticks: 5 }
        );
    }

    #[test]
    fn rim_bends_and_shift_rim_browses() {
        assert_eq!(
            run(&[&[0xB1, 0x21, 0x3D]]),
            [A::PitchBend {
                deck: DB,
                ticks: -3
            }]
        );
        let a = run(&[&[0x91, 0x3F, 0x7F], &[0xB1, 0x21, 0x3F]]);
        assert_eq!(
            a,
            [
                A::ShiftHeld {
                    deck: DB,
                    held: true
                },
                A::LibraryScroll(-1)
            ]
        );
    }

    #[test]
    fn shift_platter_searches() {
        assert_eq!(
            run(&[&[0xB0, 0x29, 0x50]]),
            [A::Search {
                deck: DA,
                ticks: 16
            }]
        );
    }

    #[test]
    fn releasing_shift_while_touching_ends_scratch() {
        let a = run(&[&[0x90, 0x36, 0x7F], &[0x90, 0x67, 0x00]]);
        assert_eq!(
            a.last(),
            Some(&A::ScratchTouch {
                deck: DA,
                touching: false
            })
        );
    }

    #[test]
    fn mixer_controls() {
        let a = run(&[
            &[0xB0, 0x07, 0x7F],
            &[0xB0, 0x27, 0x7F],
            &[0xB1, 0x0F, 0x00],
            &[0xB1, 0x2F, 0x00],
            &[0xB6, 0x17, 0x40],
            &[0xB6, 0x37, 0x00],
            &[0xB1, 0x13, 0x7F],
            &[0xB1, 0x33, 0x7F],
            &[0xB6, 0x1F, 0x00],
            &[0xB6, 0x3F, 0x00],
        ]);
        assert_eq!(
            a[0],
            A::Eq {
                deck: DA,
                band: EqBand::Hi,
                value: 1.0
            }
        );
        assert_eq!(
            a[1],
            A::Eq {
                deck: DB,
                band: EqBand::Low,
                value: 0.0
            }
        );
        assert!(matches!(a[2], A::Filter { deck: DA, value } if (value - 0.5).abs() < 0.001));
        assert_eq!(
            a[3],
            A::ChannelFader {
                deck: DB,
                value: 1.0
            }
        );
        assert_eq!(a[4], A::Crossfader(0.0));
    }

    #[test]
    fn headphone_cue_and_load() {
        assert_eq!(run(&[&[0x90, 0x54, 0x7F]]), [A::HeadphoneCue(DA)]);
        assert_eq!(run(&[&[0x91, 0x68, 0x7F]]), [A::LoadSelected(DB)]);
    }

    #[test]
    fn fader_start() {
        assert_eq!(
            run(&[&[0x90, 0x66, 0x7F]]),
            [A::FaderStart {
                deck: DA,
                play: true
            }]
        );
        assert_eq!(
            run(&[&[0x91, 0x52, 0x7F]]),
            [A::FaderStart {
                deck: DB,
                play: false
            }]
        );
    }

    #[test]
    fn effects_and_master_cue() {
        assert_eq!(run(&[&[0x96, 0x63, 0x7F]]), [A::MasterCue]);
        assert_eq!(
            run(&[&[0x96, 0x59, 0x7F], &[0x96, 0x59, 0x00]]),
            [
                A::TransitionFx { pressed: true },
                A::TransitionFx { pressed: false }
            ]
        );
        assert_eq!(run(&[&[0x96, 0x5A, 0x7F]]), [A::CycleTransitionFx]);
    }

    #[test]
    fn pads_report_shift_and_release() {
        assert_eq!(
            run(&[&[0x97, 0x02, 0x7F], &[0x98, 0x02, 0x00]]),
            [
                A::Pad {
                    deck: DA,
                    index: 2,
                    pressed: true,
                    shifted: false
                },
                A::Pad {
                    deck: DA,
                    index: 2,
                    pressed: false,
                    shifted: true
                },
            ]
        );
    }
}
