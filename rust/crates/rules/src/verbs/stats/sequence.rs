//! `stats` sequence: `cumsum`/`cummax`/`cummin`/`diffs`/`pct_changes`/`shift_arr`. All positional
//! — the output has the input's length; missing slots stay `()` (running state carries across a
//! gap; a step whose neighbor is missing — or whose divisor is 0 for `pct_changes` — is `()`).

use rhai::{Array, Engine, EvalAltResult};

use crate::grid::rhai_err;

use super::numeric::{slots, slots_to_array};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("cumsum", |a: Array| {
        let mut acc = 0.0;
        running(&a, move |x| {
            acc += x;
            acc
        })
    });
    engine.register_fn("cummax", |a: Array| {
        let mut acc = f64::NEG_INFINITY;
        running(&a, move |x| {
            acc = acc.max(x);
            acc
        })
    });
    engine.register_fn("cummin", |a: Array| {
        let mut acc = f64::INFINITY;
        running(&a, move |x| {
            acc = acc.min(x);
            acc
        })
    });
    engine.register_fn("diffs", |a: Array| step(&a, |prev, x| Some(x - prev)));
    engine.register_fn("pct_changes", |a: Array| {
        step(&a, |prev, x| {
            if prev == 0.0 {
                None // undefined step — () rather than a silent inf
            } else {
                Some((x - prev) / prev)
            }
        })
    });
    engine.register_fn("shift_arr", shift_v);
}

/// A running accumulator over numeric slots: missing slots emit `()` without feeding the state.
fn running(a: &Array, mut f: impl FnMut(f64) -> f64) -> Array {
    slots_to_array(slots(a).into_iter().map(|o| o.map(&mut f)).collect())
}

/// A neighbor-step verb: `out[i] = f(a[i-1], a[i])` when both positions are numeric, else `()`
/// (`out[0]` is always `()`).
fn step(a: &Array, f: impl Fn(f64, f64) -> Option<f64>) -> Array {
    let sl = slots(a);
    let mut out = Vec::with_capacity(sl.len());
    for i in 0..sl.len() {
        out.push(match (i.checked_sub(1).and_then(|p| sl[p]), sl[i]) {
            (Some(prev), Some(x)) => f(prev, x),
            _ => None,
        });
    }
    slots_to_array(out)
}

/// Shift the (numeric-or-`()`) slots by `n` positions: positive → right (leading `()`), negative
/// → left (trailing `()`). Output keeps the input length.
fn shift_v(a: Array, n: i64) -> Result<Array, Box<EvalAltResult>> {
    let sl = slots(&a);
    let len = sl.len() as i64;
    if n.unsigned_abs() as usize > sl.len() {
        return Err(rhai_err(format!(
            "shift_arr: shift {n} exceeds the array length ({len})"
        )));
    }
    let out = (0..len)
        .map(|i| {
            let src = i - n;
            if (0..len).contains(&src) {
                sl[src as usize]
            } else {
                None
            }
        })
        .collect();
    Ok(slots_to_array(out))
}

#[cfg(test)]
mod tests {
    use super::super::numeric::{close, fa, ga, slots};
    use super::*;

    fn cumsum_v(a: Array) -> Array {
        let mut acc = 0.0;
        running(&a, move |x| {
            acc += x;
            acc
        })
    }

    #[test]
    fn cumsum_carries_across_gaps() {
        // [1,(),2] → [1,(),3]: the gap stays (), the running sum continues.
        let out = slots(&cumsum_v(ga(&[Some(1.0), None, Some(2.0)])));
        assert_eq!(out, vec![Some(1.0), None, Some(3.0)]);
    }

    #[test]
    fn cummax_and_cummin() {
        let mut acc = f64::NEG_INFINITY;
        let mx = running(&fa(&[2.0, 1.0, 5.0]), move |x| {
            acc = acc.max(x);
            acc
        });
        assert_eq!(slots(&mx), vec![Some(2.0), Some(2.0), Some(5.0)]);
        let mut acc = f64::INFINITY;
        let mn = running(&fa(&[2.0, 1.0, 5.0]), move |x| {
            acc = acc.min(x);
            acc
        });
        assert_eq!(slots(&mn), vec![Some(2.0), Some(1.0), Some(1.0)]);
    }

    #[test]
    fn diffs_and_pct_changes_are_neighbor_steps() {
        let d = step(&fa(&[1.0, 4.0, 2.0]), |p, x| Some(x - p));
        assert_eq!(slots(&d), vec![None, Some(3.0), Some(-2.0)]);
        // A gap kills both adjacent steps.
        let d = step(&ga(&[Some(1.0), None, Some(2.0)]), |p, x| Some(x - p));
        assert_eq!(slots(&d), vec![None, None, None]);
        // pct step: (4-1)/1 = 3; a zero divisor yields ().
        let p = step(&fa(&[1.0, 4.0]), |p, x| {
            if p == 0.0 {
                None
            } else {
                Some((x - p) / p)
            }
        });
        assert!(close(slots(&p)[1].unwrap(), 3.0));
        let z = step(&fa(&[0.0, 4.0]), |p, x| {
            if p == 0.0 {
                None
            } else {
                Some((x - p) / p)
            }
        });
        assert_eq!(slots(&z), vec![None, None]);
    }

    #[test]
    fn shift_right_left_and_bounds() {
        let r = shift_v(fa(&[1.0, 2.0, 3.0]), 1).unwrap();
        assert_eq!(slots(&r), vec![None, Some(1.0), Some(2.0)]);
        let l = shift_v(fa(&[1.0, 2.0, 3.0]), -2).unwrap();
        assert_eq!(slots(&l), vec![Some(3.0), None, None]);
        assert!(shift_v(fa(&[1.0]), 2).is_err());
    }
}
