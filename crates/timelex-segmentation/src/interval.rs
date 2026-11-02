// Written by Kone Tshivhinda & Claude
//! Half-open intervals in whole Unix seconds.
//!
//! Kept private: callers speak in [`TimeBlock`](crate::TimeBlock), not in raw ranges.

/// A half-open interval `[start, end)` in whole Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Interval {
    pub(crate) start: i64,
    pub(crate) end: i64,
}

impl Interval {
    /// Whether the two half-open intervals share at least one second.
    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Sort intervals and join any whose gap is at most `gap_seconds`.
///
/// The result is sorted, disjoint and at least `gap_seconds` apart.
pub(crate) fn merge(mut intervals: Vec<Interval>, gap_seconds: i64) -> Vec<Interval> {
    intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
    let mut merged: Vec<Interval> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        match merged.last_mut() {
            Some(last) if interval.start <= last.end.saturating_add(gap_seconds) => {
                last.end = last.end.max(interval.end);
            }
            _ => merged.push(interval),
        }
    }
    merged
}

/// Remove every claimed interval from one interval.
///
/// `claimed` must be sorted and disjoint, as [`merge`] produces it.
pub(crate) fn subtract(interval: Interval, claimed: &[Interval]) -> Vec<Interval> {
    let mut remaining = vec![interval];
    for claim in claimed {
        if remaining.is_empty() {
            break;
        }
        let mut next = Vec::with_capacity(remaining.len() + 1);
        for piece in remaining {
            if claim.end <= piece.start || claim.start >= piece.end {
                next.push(piece);
                continue;
            }
            if claim.start > piece.start {
                next.push(Interval {
                    start: piece.start,
                    end: claim.start,
                });
            }
            if claim.end < piece.end {
                next.push(Interval {
                    start: claim.end,
                    end: piece.end,
                });
            }
        }
        remaining = next;
    }
    remaining
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(start: i64, end: i64) -> Interval {
        Interval { start, end }
    }

    #[test]
    fn merge_joins_gaps_at_the_tolerance_only() {
        assert_eq!(
            merge(vec![interval(0, 10), interval(15, 20)], 5),
            vec![interval(0, 20)]
        );
        assert_eq!(
            merge(vec![interval(0, 10), interval(16, 20)], 5),
            vec![interval(0, 10), interval(16, 20)]
        );
    }

    #[test]
    fn merge_handles_overlaps_and_unsorted_input() {
        assert_eq!(
            merge(vec![interval(5, 10), interval(0, 3), interval(2, 7)], 0),
            vec![interval(0, 10)]
        );
    }

    #[test]
    fn subtract_cuts_both_ends_and_middle() {
        assert_eq!(
            subtract(
                interval(0, 60),
                &[interval(-10, 10), interval(20, 30), interval(50, 70)]
            ),
            vec![interval(10, 20), interval(30, 50)]
        );
    }

    #[test]
    fn subtract_leaves_untouched_pieces_alone() {
        assert_eq!(subtract(interval(0, 10), &[]), vec![interval(0, 10)]);
        assert_eq!(
            subtract(interval(0, 10), &[interval(10, 20)]),
            vec![interval(0, 10)]
        );
    }
}
