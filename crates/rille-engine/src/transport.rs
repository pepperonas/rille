//! Transport and cue logic of one deck as a pure state machine (no audio, no time).
//!
//! Cue follows the Pioneer CDJ/rekordbox behaviour:
//! - paused away from the cue point: CUE sets the cue point at the current position;
//! - paused on the cue point: holding CUE plays from there, releasing returns and pauses;
//!   pressing PLAY while holding keeps it playing after release;
//! - playing: CUE jumps back to the cue point and pauses.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Transport {
    /// Track length in frames; positions are clamped to `0..=frames`.
    pub frames: u64,
    pub position: u64,
    pub cue: u64,
    pub playing: bool,
    /// CUE is held and the deck plays from the cue point.
    pub previewing: bool,
}

impl Transport {
    pub fn new(frames: u64) -> Transport {
        Transport {
            frames,
            ..Transport::default()
        }
    }

    fn clamp(&self, frame: u64) -> u64 {
        frame.min(self.frames)
    }

    pub fn play(&mut self) {
        if self.frames == 0 {
            return;
        }
        if self.previewing {
            // PLAY while holding CUE: keep playing once CUE is released.
            self.previewing = false;
        }
        if self.position >= self.frames {
            return;
        }
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.previewing = false;
    }

    pub fn play_pause(&mut self) {
        if self.playing && !self.previewing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn cue_press(&mut self) {
        if self.frames == 0 {
            return;
        }
        if self.playing && !self.previewing {
            self.position = self.cue;
            self.playing = false;
        } else if self.position != self.cue {
            self.cue = self.clamp(self.position);
        } else if self.position < self.frames {
            self.previewing = true;
            self.playing = true;
        }
    }

    pub fn cue_release(&mut self) {
        if self.previewing {
            self.previewing = false;
            self.playing = false;
            self.position = self.cue;
        }
    }

    /// Shift + CUE: back to the start, keeping the play state.
    pub fn jump_to_start(&mut self) {
        self.position = 0;
    }

    pub fn seek(&mut self, frame: u64) {
        self.position = self.clamp(frame);
    }

    /// Advance after rendering `frames`; stops at the end. Returns `true` when the end was hit.
    pub fn advance(&mut self, frames: u64) -> bool {
        if !self.playing {
            return false;
        }
        self.position = self.position.saturating_add(frames);
        if self.position >= self.frames {
            self.position = self.frames;
            self.playing = false;
            self.previewing = false;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paused_at(position: u64, cue: u64) -> Transport {
        Transport {
            frames: 1000,
            position,
            cue,
            ..Transport::default()
        }
    }

    #[test]
    fn cue_while_paused_away_from_cue_sets_the_cue_point() {
        let mut t = paused_at(300, 0);
        t.cue_press();
        assert_eq!(t.cue, 300);
        assert!(!t.playing);
        t.cue_release();
        assert_eq!((t.position, t.playing), (300, false));
    }

    #[test]
    fn holding_cue_on_the_cue_point_previews_and_returns() {
        let mut t = paused_at(300, 300);
        t.cue_press();
        assert!(t.playing && t.previewing);
        t.advance(50);
        t.cue_release();
        assert_eq!((t.position, t.playing, t.previewing), (300, false, false));
    }

    #[test]
    fn play_during_preview_keeps_playing_after_release() {
        let mut t = paused_at(300, 300);
        t.cue_press();
        t.advance(50);
        t.play();
        t.cue_release();
        assert!(t.playing);
        assert_eq!(t.position, 350);
    }

    #[test]
    fn cue_while_playing_jumps_back_and_pauses() {
        let mut t = paused_at(100, 100);
        t.play();
        t.advance(400);
        t.cue_press();
        assert_eq!((t.position, t.playing), (100, false));
        t.cue_release();
        assert_eq!((t.position, t.playing), (100, false));
    }

    #[test]
    fn jump_to_start_keeps_play_state() {
        let mut t = paused_at(500, 0);
        t.play();
        t.jump_to_start();
        assert_eq!((t.position, t.playing), (0, true));
    }

    #[test]
    fn cue_and_seek_are_clamped_to_the_track() {
        let mut t = paused_at(0, 0);
        t.seek(5000);
        assert_eq!(t.position, 1000);
        t.cue_press();
        assert_eq!(t.cue, 1000);
    }

    #[test]
    fn preview_at_the_very_end_does_not_start() {
        let mut t = paused_at(1000, 1000);
        t.cue_press();
        assert!(!t.playing);
    }

    #[test]
    fn playback_stops_at_the_end() {
        let mut t = paused_at(990, 0);
        t.play();
        assert!(t.advance(20));
        assert_eq!((t.position, t.playing), (1000, false));
        t.play();
        assert!(!t.playing, "cannot play past the end");
    }

    #[test]
    fn empty_deck_ignores_everything() {
        let mut t = Transport::new(0);
        t.play();
        t.cue_press();
        assert!(!t.playing && !t.previewing);
    }

    #[test]
    fn play_pause_toggles_but_not_during_preview() {
        let mut t = paused_at(0, 0);
        t.play_pause();
        assert!(t.playing);
        t.play_pause();
        assert!(!t.playing);
        t.cue_press(); // preview
        t.play_pause(); // acts like PLAY: commit to playing
        t.cue_release();
        assert!(t.playing);
    }
}
