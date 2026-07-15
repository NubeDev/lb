//! `stats` missing-value verbs: `dropna`/`fillna`/`ffill`/`bfill`/`interp_linear`. These are the
//! policy's repair tools: `dropna` compacts to the numeric values; the fills are positional and
//! keep the input length (slots they cannot fill — leading for `ffill`, trailing for `bfill`,
//! either edge for `interp_linear` — stay `()`).

use rhai::{Array, Dynamic, Engine, EvalAltResult};

use super::numeric::{floats_to_array, nums, scalar, slots, slots_to_array};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("dropna", |a: Array| floats_to_array(nums(&a)));
    engine.register_fn("fillna", fillna_v);
    engine.register_fn("ffill", |a: Array| {
        let mut last = None;
        slots_to_array(
            slots(&a)
                .into_iter()
                .map(|o| {
                    if o.is_some() {
                        last = o;
                    }
                    last
                })
                .collect(),
        )
    });
    engine.register_fn("bfill", |a: Array| {
        let mut next = None;
        let mut out: Vec<Option<f64>> = slots(&a)
            .into_iter()
            .rev()
            .map(|o| {
                if o.is_some() {
                    next = o;
                }
                next
            })
            .collect();
        out.reverse();
        slots_to_array(out)
    });
    engine.register_fn("interp_linear", interp_v);
}

/// Replace every missing slot with `v` (a number — the family's arrays stay numeric-or-`()`).
fn fillna_v(a: Array, v: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let fill = scalar(&v, "fillna", "v")?;
    Ok(slots_to_array(
        slots(&a)
            .into_iter()
            .map(|o| Some(o.unwrap_or(fill)))
            .collect(),
    ))
}

/// Linear interpolation across interior gaps (position-weighted between the nearest numeric
/// neighbors); leading/trailing gaps have no pair to interpolate and stay `()`.
fn interp_v(a: Array) -> Array {
    let sl = slots(&a);
    let mut out = sl.clone();
    let mut prev: Option<usize> = None;
    for i in 0..sl.len() {
        if sl[i].is_some() {
            if let Some(p) = prev {
                if i > p + 1 {
                    let (x0, x1) = (sl[p].unwrap(), sl[i].unwrap());
                    for (k, slot) in out.iter_mut().enumerate().take(i).skip(p + 1) {
                        let t = (k - p) as f64 / (i - p) as f64;
                        *slot = Some(x0 + (x1 - x0) * t);
                    }
                }
            }
            prev = Some(i);
        }
    }
    slots_to_array(out)
}

#[cfg(test)]
mod tests {
    use super::super::numeric::{fa, ga, slots};
    use super::*;

    #[test]
    fn dropna_compacts_to_numeric_only() {
        let out = super::super::numeric::nums(&ga(&[None, Some(1.0), None, Some(3.5)]));
        assert_eq!(out, vec![1.0, 3.5]);
    }

    #[test]
    fn fillna_replaces_missing_and_rejects_non_numeric_fill() {
        let out = fillna_v(ga(&[Some(1.0), None]), Dynamic::from_float(9.0)).unwrap();
        assert_eq!(slots(&out), vec![Some(1.0), Some(9.0)]);
        assert!(fillna_v(fa(&[1.0]), Dynamic::from("x")).is_err());
    }

    #[test]
    fn ffill_bfill_leave_unfillable_edges() {
        let gaps = || ga(&[None, Some(1.0), None, Some(3.0), None]);
        let mut last = None;
        let f = slots_to_array(
            slots(&gaps())
                .into_iter()
                .map(|o| {
                    if o.is_some() {
                        last = o;
                    }
                    last
                })
                .collect(),
        );
        assert_eq!(
            slots(&f),
            vec![None, Some(1.0), Some(1.0), Some(3.0), Some(3.0)]
        );
        let mut next = None;
        let mut b: Vec<Option<f64>> = slots(&gaps())
            .into_iter()
            .rev()
            .map(|o| {
                if o.is_some() {
                    next = o;
                }
                next
            })
            .collect();
        b.reverse();
        assert_eq!(b, vec![Some(1.0), Some(1.0), Some(3.0), Some(3.0), None]);
    }

    #[test]
    fn interp_linear_interior_gaps_only() {
        // [(),1,(),(),4,()] → edges stay (), the interior run interpolates 2, 3.
        let out = interp_v(ga(&[None, Some(1.0), None, None, Some(4.0), None]));
        assert_eq!(
            slots(&out),
            vec![None, Some(1.0), Some(2.0), Some(3.0), Some(4.0), None]
        );
    }
}
