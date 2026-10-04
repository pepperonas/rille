/// One of the two decks. Deck A sits left, deck B right, as on the DDJ-200.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}

impl DeckId {
    pub const ALL: [DeckId; 2] = [DeckId::A, DeckId::B];

    /// Zero-based index, usable for fixed-size per-deck arrays.
    pub const fn index(self) -> usize {
        match self {
            DeckId::A => 0,
            DeckId::B => 1,
        }
    }

    pub const fn from_index(index: usize) -> Option<DeckId> {
        match index {
            0 => Some(DeckId::A),
            1 => Some(DeckId::B),
            _ => None,
        }
    }

    pub const fn other(self) -> DeckId {
        match self {
            DeckId::A => DeckId::B,
            DeckId::B => DeckId::A,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trips() {
        for deck in DeckId::ALL {
            assert_eq!(DeckId::from_index(deck.index()), Some(deck));
        }
        assert_eq!(DeckId::from_index(2), None);
    }

    #[test]
    fn other_swaps_sides() {
        assert_eq!(DeckId::A.other(), DeckId::B);
        assert_eq!(DeckId::B.other(), DeckId::A);
    }
}
