//! A trend kept over the whole of a take while it is still being heard
//! (`record --live`), in a bounded number of points (#2318).

/// Adds `point`, a reading heard at `t_s(&point)` seconds into the take,
/// to a trend kept over the whole take in at most `max` points.
///
/// `kept` holds at most one point in each `step_s` of the take, the first
/// heard in it, and at most `max - 1` points. When one more would be held,
/// `step_s` doubles and each wider step keeps its first point, so the kept
/// points stay one step apart over the whole take, the take's first point
/// among them. Returns the point when it is not among `kept`: the newest
/// reading, for the caller to give after them, so the trend always ends at
/// the latest reading.
pub fn keep_trend_point<T: Clone>(
    kept: &mut Vec<T>,
    step_s: &mut f64,
    max: usize,
    t_s: impl Fn(&T) -> f64,
    point: T,
) -> Option<T> {
    let slot = |p: &T, step: f64| (t_s(p) / step).floor();
    let fresh = kept
        .last()
        .is_none_or(|last| slot(&point, *step_s) > slot(last, *step_s));
    if !fresh {
        return Some(point);
    }
    let t = t_s(&point);
    kept.push(point.clone());
    while kept.len() > max.saturating_sub(1).max(1) {
        *step_s *= 2.0;
        let step = *step_s;
        // `dedup_by` drops the later of two neighbours in one step.
        kept.dedup_by(|later, earlier| slot(later, step) == slot(earlier, step));
    }
    match kept.last() {
        Some(last) if t_s(last) == t => None,
        _ => Some(point),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The points given: the kept ones, then the newest if it is not kept.
    fn given(kept: &[f64], newest: Option<f64>) -> Vec<f64> {
        kept.iter().copied().chain(newest).collect()
    }

    /// Two hours heard a second at a time keep at most 300 points, one
    /// step apart over the whole take, the first and the newest among them
    /// (#2318: the rule before kept the first and then only the last ~11
    /// minutes).
    #[test]
    fn a_long_take_keeps_its_whole_span_evenly() {
        let (mut kept, mut step) = (Vec::new(), 1.0);
        let end = 7200;
        for s in 1..=end {
            let newest = keep_trend_point(&mut kept, &mut step, 300, |t: &f64| *t, s as f64);
            let points = given(&kept, newest);
            assert!(points.len() <= 300, "{} points at {s} s", points.len());
            assert_eq!(points[0], 1.0, "the first point stays");
            assert_eq!(*points.last().unwrap(), s as f64, "the newest is given");
        }
        assert_eq!(step, 32.0);
        assert!(kept.len() > 150, "only {} kept", kept.len());
        for w in kept.windows(2).skip(1) {
            assert_eq!(w[1] - w[0], step, "uneven at {} s", w[0]);
        }
        // The first point sits in the first step; the next starts the second.
        assert_eq!(kept[1], step);
        assert!(*kept.last().unwrap() > (end as f64) - step);
        // The middle of the take is covered as well as its end.
        assert!(kept.iter().any(|&t| (t - 3600.0).abs() <= step));
    }

    /// A reading in a step already kept is given as the newest and not
    /// kept, and a later one replaces it; no reading heard means no point.
    #[test]
    fn a_second_reading_in_one_step_is_only_the_newest() {
        let (mut kept, mut step) = (Vec::new(), 4.0);
        let f = |t: &f64| *t;
        assert_eq!(keep_trend_point(&mut kept, &mut step, 300, f, 1.0), None);
        assert_eq!(
            keep_trend_point(&mut kept, &mut step, 300, f, 2.0),
            Some(2.0)
        );
        assert_eq!(
            keep_trend_point(&mut kept, &mut step, 300, f, 3.5),
            Some(3.5)
        );
        assert_eq!(keep_trend_point(&mut kept, &mut step, 300, f, 9.0), None);
        assert_eq!(kept, vec![1.0, 9.0]);
        assert_eq!(step, 4.0);
    }

    /// When thinning drops the point just added, it comes back as the
    /// newest, so the trend still ends at the latest reading.
    #[test]
    fn a_point_thinned_on_arrival_is_still_given() {
        let (mut kept, mut step) = (vec![0.0, 1.0, 2.0], 1.0);
        let newest = keep_trend_point(&mut kept, &mut step, 4, |t: &f64| *t, 3.0);
        assert_eq!(step, 2.0);
        assert_eq!(kept, vec![0.0, 2.0]);
        assert_eq!(newest, Some(3.0));
    }
}
