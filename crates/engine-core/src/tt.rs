//! The transposition table: what earlier searches learned about a position, found
//! again by its hash key.
//!
//! Every slot is two atomic words, the packed entry and the key XORed with it. A probe
//! recomputes the XOR and rejects anything that does not give the key back, so a slot
//! written by two threads at once reads as a miss and the access path needs no lock
//! and no `unsafe`. Four slots share one cache line.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering::Relaxed};

use crate::Move;
use crate::search::{MATE, MATE_BOUND};

/// What a stored score says about the true score of its position.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Bound {
    /// Nothing; the entry carries only a static evaluation.
    None = 0,
    /// The true score is at most the stored one.
    Upper = 1,
    /// The true score is at least the stored one.
    Lower = 2,
    Exact = 3,
}

impl Bound {
    fn from_bits(bits: u64) -> Bound {
        match bits & 3 {
            0 => Bound::None,
            1 => Bound::Upper,
            2 => Bound::Lower,
            _ => Bound::Exact,
        }
    }

    /// Whether a score with this bound settles a search with the window `alpha..beta`.
    #[inline]
    pub fn cuts(self, score: i32, alpha: i32, beta: i32) -> bool {
        match self {
            Bound::None => false,
            Bound::Upper => score <= alpha,
            Bound::Lower => score >= beta,
            Bound::Exact => true,
        }
    }
}

/// Stored in place of a static evaluation the node never computed.
pub const NO_EVAL: i32 = i16::MIN as i32;

/// Depths are stored with this offset so that zero can mean an empty slot.
const DEPTH_OFFSET: i32 = 7;
const GENERATION_MASK: u8 = 31;

/// One stored result.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TtEntry {
    /// The best move found, unvalidated: a key collision can produce a move that is
    /// not playable in the probing position.
    pub mv: Option<Move>,
    /// As stored, relative to the node. [`from_tt`] converts it for use.
    pub score: i32,
    /// The static evaluation, or [`NO_EVAL`].
    pub eval: i32,
    pub depth: i32,
    pub bound: Bound,
    /// Whether the position has been part of a principal variation.
    pub pv: bool,
}

impl TtEntry {
    /// Bits 0–15 move, 16–31 score, 32–47 static evaluation, 48–55 depth plus
    /// [`DEPTH_OFFSET`], 56–57 bound, 58 the PV flag, 59–63 generation.
    fn pack(&self, generation: u8) -> u64 {
        debug_assert!(i16::try_from(self.score).is_ok() && i16::try_from(self.eval).is_ok());
        debug_assert!((1..=255).contains(&(self.depth + DEPTH_OFFSET)));
        self.mv.map_or(0, Move::raw) as u64
            | ((self.score as i16 as u16 as u64) << 16)
            | ((self.eval as i16 as u16 as u64) << 32)
            | (((self.depth + DEPTH_OFFSET) as u8 as u64) << 48)
            | ((self.bound as u64) << 56)
            | ((self.pv as u64) << 58)
            | (((generation & GENERATION_MASK) as u64) << 59)
    }

    fn unpack(data: u64) -> TtEntry {
        TtEntry {
            mv: Move::from_raw(data as u16),
            score: (data >> 16) as u16 as i16 as i32,
            eval: (data >> 32) as u16 as i16 as i32,
            depth: ((data >> 48) & 0xFF) as i32 - DEPTH_OFFSET,
            bound: Bound::from_bits(data >> 56),
            pv: (data >> 58) & 1 != 0,
        }
    }
}

fn generation_of(data: u64) -> u8 {
    (data >> 59) as u8
}

fn depth_of(data: u64) -> i32 {
    ((data >> 48) & 0xFF) as i32
}

/// A mate score counts plies from the root; the table stores it counted from the node,
/// so that it stays true wherever in a later search the position turns up.
#[inline]
pub fn to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_BOUND {
        score + ply as i32
    } else if score <= -MATE_BOUND {
        score - ply as i32
    } else {
        score
    }
}

/// The inverse of [`to_tt`]. A mate too far away to be delivered before the fifty-move
/// rule ends the game is reported as a large score that is not a mate.
#[inline]
pub fn from_tt(score: i32, ply: usize, halfmove: u8) -> i32 {
    let left = 100 - halfmove as i32;
    if score >= MATE_BOUND {
        if MATE - score > left {
            MATE_BOUND - 1
        } else {
            score - ply as i32
        }
    } else if score <= -MATE_BOUND {
        if MATE + score > left {
            -MATE_BOUND + 1
        } else {
            score + ply as i32
        }
    } else {
        score
    }
}

#[derive(Default)]
#[repr(C)]
struct Slot {
    data: AtomicU64,
    /// The position's key XORed with `data`.
    check: AtomicU64,
}

#[derive(Default)]
#[repr(C, align(64))]
struct Cluster([Slot; 4]);

const _: () = assert!(size_of::<Cluster>() == 64);

pub struct TranspositionTable {
    clusters: Box<[Cluster]>,
    /// Counts searches, so that entries left by earlier ones can be told apart.
    generation: AtomicU8,
}

impl TranspositionTable {
    pub const DEFAULT_MB: usize = 16;
    pub const MAX_MB: usize = 262_144;

    /// A table of `megabytes`, or as near as memory allows.
    pub fn new(megabytes: usize) -> TranspositionTable {
        let mut table = TranspositionTable {
            clusters: Box::default(),
            generation: AtomicU8::new(0),
        };
        table.resize(megabytes);
        table
    }

    /// Replaces the table with an empty one of `megabytes` and returns the size
    /// obtained, which is smaller when the request cannot be allocated. The old table
    /// is freed first, so the two never exist together.
    pub fn resize(&mut self, megabytes: usize) -> usize {
        self.clusters = Box::default();
        let mut megabytes = megabytes.clamp(1, Self::MAX_MB);
        loop {
            let count = megabytes * (1 << 20) / size_of::<Cluster>();
            let mut clusters = Vec::new();
            if clusters.try_reserve_exact(count).is_ok() {
                clusters.resize_with(count, Cluster::default);
                self.clusters = clusters.into_boxed_slice();
                self.generation.store(0, Relaxed);
                return megabytes;
            }
            megabytes = (megabytes / 2).max(1);
        }
    }

    pub fn megabytes(&self) -> usize {
        (self.clusters.len() * size_of::<Cluster>()) >> 20
    }

    /// Forgets everything.
    pub fn clear(&self) {
        for cluster in &self.clusters {
            for slot in &cluster.0 {
                slot.data.store(0, Relaxed);
                slot.check.store(0, Relaxed);
            }
        }
        self.generation.store(0, Relaxed);
    }

    /// Marks the start of a new search.
    pub fn new_search(&self) {
        let next = self.generation.load(Relaxed).wrapping_add(1) & GENERATION_MASK;
        self.generation.store(next, Relaxed);
    }

    /// Maps the key onto the table without requiring a power-of-two size.
    #[inline]
    fn cluster(&self, key: u64) -> &Cluster {
        let index = ((key as u128 * self.clusters.len() as u128) >> 64) as usize;
        &self.clusters[index]
    }

    /// The entry stored for `key`, if any. A hit from an earlier search is marked as
    /// belonging to this one, which protects it from replacement.
    #[inline]
    pub fn probe(&self, key: u64) -> Option<TtEntry> {
        let generation = self.generation.load(Relaxed);
        for slot in &self.cluster(key).0 {
            let data = slot.data.load(Relaxed);
            if data != 0 && slot.check.load(Relaxed) ^ data == key {
                if generation_of(data) != generation {
                    let fresh =
                        (data & !((GENERATION_MASK as u64) << 59)) | ((generation as u64) << 59);
                    slot.data.store(fresh, Relaxed);
                    slot.check.store(key ^ fresh, Relaxed);
                }
                return Some(TtEntry::unpack(data));
            }
        }
        None
    }

    /// Records `entry` for `key`. `entry.score` must already be converted with
    /// [`to_tt`].
    pub fn store(&self, key: u64, mut entry: TtEntry) {
        let generation = self.generation.load(Relaxed);
        let cluster = self.cluster(key);

        let mut target = &cluster.0[0];
        let mut lowest = i32::MAX;
        for slot in &cluster.0 {
            let data = slot.data.load(Relaxed);
            if data != 0 && slot.check.load(Relaxed) ^ data == key {
                let old = TtEntry::unpack(data);
                if entry.mv.is_none() {
                    entry.mv = old.mv;
                }
                // A shallower result does not displace a deeper one from this search
                // unless it is exact.
                let keep = entry.bound != Bound::Exact
                    && generation_of(data) == generation
                    && entry.depth + 4 + 2 * entry.pv as i32 <= old.depth;
                if keep {
                    // The move is still worth having for ordering.
                    if entry.mv != old.mv {
                        let patched = (data & !0xFFFF) | entry.mv.map_or(0, Move::raw) as u64;
                        slot.data.store(patched, Relaxed);
                        slot.check.store(key ^ patched, Relaxed);
                    }
                    return;
                }
                target = slot;
                break;
            }
            // Empty slots have a stored depth of zero and so rank lowest of all.
            let age = generation.wrapping_sub(generation_of(data)) & GENERATION_MASK;
            let worth = if data == 0 {
                i32::MIN
            } else {
                depth_of(data) - 8 * age as i32
            };
            if worth < lowest {
                lowest = worth;
                target = slot;
            }
        }

        let data = entry.pack(generation);
        target.data.store(data, Relaxed);
        target.check.store(key ^ data, Relaxed);
    }

    /// How full the table is with entries from the current search, in parts per
    /// thousand, estimated from the first thousand slots.
    pub fn hashfull(&self) -> usize {
        let generation = self.generation.load(Relaxed);
        let sample = &self.clusters[..self.clusters.len().min(250)];
        let used = sample
            .iter()
            .flat_map(|cluster| &cluster.0)
            .filter(|slot| {
                let data = slot.data.load(Relaxed);
                data != 0 && generation_of(data) == generation
            })
            .count();
        used * 1000 / (sample.len() * 4).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MoveFlag, Square};

    fn entry(depth: i32, bound: Bound) -> TtEntry {
        TtEntry {
            mv: Some(Move::new(Square::E2, Square::E4, MoveFlag::DoublePush)),
            score: 37,
            eval: -12,
            depth,
            bound,
            pv: false,
        }
    }

    #[test]
    fn pack_round_trip() {
        let bounds = [Bound::None, Bound::Upper, Bound::Lower, Bound::Exact];
        let scores = [0, 1, -1, 250, -250, MATE, -MATE, MATE_BOUND, -MATE_BOUND];
        for (i, &score) in scores.iter().enumerate() {
            for depth in [-6, -1, 0, 1, 17, 240] {
                for generation in [0, 1, 17, 31] {
                    let e = TtEntry {
                        mv: Move::from_raw((i as u16 * 4099).wrapping_add(depth as u16)),
                        score,
                        eval: if i % 2 == 0 { NO_EVAL } else { -score / 2 },
                        depth,
                        bound: bounds[i % 4],
                        pv: i % 3 == 0,
                    };
                    let data = e.pack(generation);
                    assert_ne!(data, 0);
                    assert_eq!(TtEntry::unpack(data), e);
                    assert_eq!(generation_of(data), generation);
                }
            }
        }
    }

    #[test]
    fn mate_scores_round_trip_through_the_table() {
        for ply in [0, 1, 7, 100] {
            for score in [0, 55, -900, MATE_BOUND - 1, -MATE_BOUND + 1] {
                assert_eq!(to_tt(score, ply), score);
                assert_eq!(from_tt(score, ply, 0), score);
            }
            for distance in [ply as i32, ply as i32 + 1, ply as i32 + 20] {
                for score in [MATE - distance, -MATE + distance] {
                    assert_eq!(from_tt(to_tt(score, ply), ply, 0), score);
                }
            }
        }
    }

    #[test]
    fn mates_beyond_the_fifty_move_horizon_are_not_mates() {
        // Mate in 12 plies from the node, with 10 plies left on the counter.
        let stored = MATE - 12;
        assert_eq!(from_tt(stored, 3, 90), MATE_BOUND - 1);
        assert_eq!(from_tt(-stored, 3, 90), -MATE_BOUND + 1);
        // With 12 plies left it can still be delivered.
        assert_eq!(from_tt(stored, 3, 88), stored - 3);
    }

    #[test]
    fn store_and_probe() {
        let tt = TranspositionTable::new(1);
        assert_eq!(tt.megabytes(), 1);
        assert_eq!(tt.probe(0x1234_5678_9ABC_DEF0), None);
        let e = entry(5, Bound::Exact);
        tt.store(0x1234_5678_9ABC_DEF0, e);
        assert_eq!(tt.probe(0x1234_5678_9ABC_DEF0), Some(e));
        // Same cluster (the high bits pick it), different key.
        assert_eq!(tt.probe(0x1234_5678_9ABC_DEF1), None);
        tt.clear();
        assert_eq!(tt.probe(0x1234_5678_9ABC_DEF0), None);
    }

    #[test]
    fn a_torn_slot_reads_as_a_miss() {
        let tt = TranspositionTable::new(1);
        let key = 0xDEAD_BEEF_0BAD_F00D;
        tt.store(key, entry(5, Bound::Exact));
        let slot = &tt.cluster(key).0[0];
        slot.data.store(entry(9, Bound::Lower).pack(0), Relaxed);
        assert_eq!(tt.probe(key), None);
    }

    #[test]
    fn same_key_keeps_the_deeper_result_and_the_move() {
        let tt = TranspositionTable::new(1);
        let key = 42 << 40;
        tt.store(key, entry(12, Bound::Lower));
        // Much shallower and not exact: ignored.
        tt.store(key, entry(3, Bound::Upper));
        assert_eq!(tt.probe(key).unwrap().depth, 12);
        // Exact always replaces, and a missing move keeps the old one.
        let mut exact = entry(3, Bound::Exact);
        exact.mv = None;
        tt.store(key, exact);
        let found = tt.probe(key).unwrap();
        assert_eq!((found.depth, found.bound), (3, Bound::Exact));
        assert_eq!(found.mv, entry(0, Bound::None).mv);
    }

    #[test]
    fn a_full_cluster_gives_up_its_shallowest_or_oldest_entry() {
        let tt = TranspositionTable::new(1);
        // Keys that differ only in the low bits share a cluster.
        let key = |i: u64| (7 << 50) | i;
        for i in 0..4 {
            tt.store(key(i), entry(10 + i as i32, Bound::Lower));
        }
        tt.store(key(4), entry(1, Bound::Lower));
        assert_eq!(tt.probe(key(0)), None);
        assert!((1..=4).all(|i| tt.probe(key(i)).is_some()));

        // After a new search starts, old entries lose eight plies of worth per search,
        // except those a probe has touched.
        tt.new_search();
        tt.new_search();
        for i in 1..4 {
            tt.probe(key(i));
        }
        tt.store(key(5), entry(2, Bound::Lower));
        assert_eq!(tt.probe(key(4)), None);
        assert!([1, 2, 3, 5].iter().all(|&i| tt.probe(key(i)).is_some()));
    }

    #[test]
    fn hashfull_counts_only_the_current_search() {
        let tt = TranspositionTable::new(1);
        assert_eq!(tt.hashfull(), 0);
        // The first 250 clusters hold the keys with the smallest high bits.
        let clusters = tt.clusters.len() as u64;
        for i in 0..100u64 {
            tt.store(i * (u64::MAX / clusters + 1), entry(1, Bound::Lower));
        }
        assert_eq!(tt.hashfull(), 100);
        tt.new_search();
        assert_eq!(tt.hashfull(), 0);
    }

    #[test]
    fn resize_empties_the_table() {
        let mut tt = TranspositionTable::new(2);
        tt.store(99, entry(4, Bound::Exact));
        assert_eq!(tt.resize(1), 1);
        assert_eq!(tt.megabytes(), 1);
        assert_eq!(tt.probe(99), None);
    }
}
