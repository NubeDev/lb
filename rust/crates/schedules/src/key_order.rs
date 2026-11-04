//! The order overlapping entries that started at the same minute are ranked in, inside one map.
//! rubix-os has none there: `CheckWeeklyScheduleCollection` (`src/schedule/weekly.go` lines 110 to
//! 128) and the event collection fold a Go map, whose iteration order is random, and
//! `CombineScheduleCheckerResults` (`src/schedule/schedule_utils.go` lines 198 to 204) keeps
//! whichever result it already holds on a tie. (Between the maps rubix-os does decide: weekly beats
//! event, `services/schedule/schedule.go` line 55; `evaluate.rs` applies that before this.) lb orders
//! keys the way people read them, `w2` before `w10`, so its answer is at least stable.

use std::cmp::Ordering;

/// `a` before `b` in natural order: runs of digits compare as numbers, everything else by character.
/// Keys that differ only in leading zeros (`w02`, `w2`) fall back to plain string order.
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural(a, b).then_with(|| a.cmp(b))
}

fn natural(mut a: &str, mut b: &str) -> Ordering {
    loop {
        let (Some(x), Some(y)) = (a.chars().next(), b.chars().next()) else {
            return a.len().cmp(&b.len());
        };
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let (da, ra) = split_digits(a);
            let (db, rb) = split_digits(b);
            let (da, db) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
            let ord = da.len().cmp(&db.len()).then_with(|| da.cmp(db));
            if ord != Ordering::Equal {
                return ord;
            }
            (a, b) = (ra, rb);
        } else {
            if x != y {
                return x.cmp(&y);
            }
            (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
        }
    }
}

/// The leading run of ASCII digits of `s`, and the rest.
fn split_digits(s: &str) -> (&str, &str) {
    s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()))
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;
    use std::cmp::Ordering::{Equal, Greater, Less};

    #[test]
    fn digits_compare_as_numbers() {
        assert_eq!(natural_cmp("w2", "w10"), Less);
        assert_eq!(natural_cmp("w10", "w2"), Greater);
        assert_eq!(natural_cmp("a", "b"), Less);
        assert_eq!(natural_cmp("w", "w1"), Less);
        assert_eq!(natural_cmp("w1", "w1"), Equal);
    }

    #[test]
    fn leading_zeros_fall_back_to_string_order() {
        assert_eq!(natural_cmp("w02", "w2"), Less);
    }
}
