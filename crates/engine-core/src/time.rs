//! Search limits and how much clock time a move may use.

use std::time::Duration;

use crate::Color;

/// What a `go` command asked for. Everything unset means search until told to stop.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Limits {
    /// Stop after completing this depth.
    pub depth: Option<i32>,
    /// Stop after this many nodes.
    pub nodes: Option<u64>,
    /// Use exactly this many milliseconds.
    pub movetime: Option<u64>,
    /// Milliseconds left on each side's clock, indexed by colour.
    pub time: [Option<u64>; 2],
    /// Milliseconds added per move, indexed by colour.
    pub inc: [u64; 2],
    /// Moves until the next time control, if there is one.
    pub movestogo: Option<u64>,
    /// Never stop without being told to, even if the search runs out of depth.
    pub infinite: bool,
}

impl Limits {
    pub fn depth(depth: i32) -> Limits {
        Limits {
            depth: Some(depth),
            ..Limits::default()
        }
    }

    /// Whether the search is being timed by a clock or a fixed move time.
    pub fn is_timed(&self, stm: Color) -> bool {
        self.movetime.is_some() || self.time[stm.index()].is_some()
    }
}

/// Time allowed for one move, measured from the start of the search.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct TimeBudget {
    /// Do not start another iteration after this.
    pub soft: Option<Duration>,
    /// Abort the search at this point.
    pub hard: Option<Duration>,
}

impl TimeBudget {
    /// The budget for `stm` under `limits`, keeping `overhead_ms` in reserve for the
    /// delay between deciding on a move and the clock stopping.
    pub fn new(limits: &Limits, stm: Color, overhead_ms: u64) -> TimeBudget {
        if let Some(movetime) = limits.movetime {
            let ms = movetime.saturating_sub(overhead_ms).max(1);
            let exact = Some(Duration::from_millis(ms));
            return TimeBudget {
                soft: exact,
                hard: exact,
            };
        }
        let Some(remaining) = limits.time[stm.index()] else {
            return TimeBudget::default();
        };

        let t = remaining.saturating_sub(overhead_ms).max(1);
        let inc = limits.inc[stm.index()];
        let moves_left = match limits.movestogo {
            Some(moves) if moves > 0 => moves.min(40) + 1,
            _ => 20,
        };
        let base = t / moves_left + 3 * inc / 4;
        let hard = (4 * base).min(3 * t / 4).max(1);
        let soft = (13 * base / 20).clamp(1, hard);
        TimeBudget {
            soft: Some(Duration::from_millis(soft)),
            hard: Some(Duration::from_millis(hard)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(time: u64, inc: u64, movestogo: Option<u64>) -> Limits {
        Limits {
            time: [Some(time); 2],
            inc: [inc; 2],
            movestogo,
            ..Limits::default()
        }
    }

    fn ms(budget: TimeBudget) -> (u64, u64) {
        (
            budget.soft.unwrap().as_millis() as u64,
            budget.hard.unwrap().as_millis() as u64,
        )
    }

    #[test]
    fn untimed_limits_have_no_budget() {
        assert_eq!(
            TimeBudget::new(&Limits::default(), Color::White, 10),
            TimeBudget::default()
        );
        assert_eq!(
            TimeBudget::new(&Limits::depth(5), Color::Black, 10),
            TimeBudget::default()
        );
        assert!(!Limits::depth(5).is_timed(Color::White));
    }

    #[test]
    fn movetime_is_used_exactly_minus_overhead() {
        let limits = Limits {
            movetime: Some(1000),
            ..Limits::default()
        };
        assert_eq!(ms(TimeBudget::new(&limits, Color::White, 10)), (990, 990));
        let tiny = Limits {
            movetime: Some(5),
            ..Limits::default()
        };
        assert_eq!(ms(TimeBudget::new(&tiny, Color::White, 10)), (1, 1));
        assert!(limits.is_timed(Color::Black));
    }

    #[test]
    fn sudden_death_with_increment() {
        // t = 7990, base = 399 + 60 = 459, hard = min(1836, 5992), soft = 298.
        assert_eq!(
            ms(TimeBudget::new(&clock(8000, 80, None), Color::White, 10)),
            (298, 1836)
        );
    }

    #[test]
    fn moves_to_go() {
        // t = 9990 over 9 + 1 moves: base = 999, hard = min(3996, 7492), soft = 649.
        assert_eq!(
            ms(TimeBudget::new(
                &clock(10_000, 0, Some(9)),
                Color::Black,
                10
            )),
            (649, 3996)
        );
        // The last move before the control never uses more than three quarters.
        let (soft, hard) = ms(TimeBudget::new(
            &clock(10_000, 0, Some(1)),
            Color::White,
            10,
        ));
        assert!(soft <= hard && hard <= 7_500);
    }

    #[test]
    fn nearly_flagged_clock_stays_inside_the_remaining_time() {
        for remaining in [1, 5, 11, 20, 50, 100] {
            let (soft, hard) = ms(TimeBudget::new(
                &clock(remaining, 80, None),
                Color::White,
                10,
            ));
            assert!(soft >= 1 && soft <= hard, "{remaining}");
            assert!(hard <= remaining.max(1), "{remaining}: hard {hard}");
        }
    }

    #[test]
    fn each_side_uses_its_own_clock() {
        let limits = Limits {
            time: [Some(60_000), Some(1_000)],
            ..Limits::default()
        };
        let white = ms(TimeBudget::new(&limits, Color::White, 0));
        let black = ms(TimeBudget::new(&limits, Color::Black, 0));
        assert!(white.0 > black.0 && white.1 > black.1);
    }
}
