//! Where a take's readings hear no watch: the stretches whose beats are
//! the stand's noise, not the watch's, so the strip draws them as possible
//! beats and fits its width to the watch's alone (#2177).

use super::TrendReading;

/// The stretches of a take from 0 to `end_s` that no reading of `trend`
/// with a watch heard ([`TrendReading::heard`]) covers, in time order. A
/// reading at `t` covers the `average_s` before it; two readings in a row
/// with a watch heard cover all between them too, however far apart a
/// long take's trend spaces them. What is after the last reading counts
/// as that reading does. With no reading at all, nothing is known, so none.
pub fn unheard_spans(trend: &[TrendReading], average_s: f64, end_s: f64) -> Vec<(f64, f64)> {
    let Some(last) = trend.last() else {
        return Vec::new();
    };
    let mut heard: Vec<(f64, f64)> = Vec::new();
    let mut run: Option<(f64, f64)> = None;
    for p in trend {
        run = match (run, p.heard) {
            (Some((a, _)), true) => Some((a, p.t)),
            (None, true) => Some(((p.t - average_s).max(0.0), p.t)),
            (Some(r), false) => {
                heard.push(r);
                None
            }
            (None, false) => None,
        };
    }
    if let Some((a, _)) = run {
        heard.push((a, end_s.max(last.t)));
    }
    let mut out = Vec::new();
    let mut from = 0.0;
    for (a, b) in heard {
        if a > from {
            out.push((from, a));
        }
        from = from.max(b);
    }
    if from < end_s {
        out.push((from, end_s));
    }
    out
}

/// Whether `t` lies in one of `spans` ([`unheard_spans`]): a beat there
/// is a possible beat, not the watch's.
pub fn in_spans(spans: &[(f64, f64)], t: f64) -> bool {
    spans.iter().any(|&(a, b)| t >= a && t <= b)
}

/// The parts of the stretches `spans` (start, end; the last may run on)
/// up to `end_s` that a watch was heard in, in time order: each stretch
/// less every stretch [`unheard_spans`] gives for `trend`. With no reading
/// at all none of it is known to hold a watch, so none (#2183: silence is
/// not beats). The one set of heard stretches a position's steadiness
/// tests start on, read and count (Norm on #2451).
pub fn heard_spans(
    trend: &[TrendReading],
    average_s: f64,
    spans: &[(f64, f64)],
    end_s: f64,
) -> Vec<(f64, f64)> {
    if trend.is_empty() {
        return Vec::new();
    }
    let unheard = unheard_spans(trend, average_s, end_s);
    let mut out = Vec::new();
    for (a, b) in spans
        .iter()
        .map(|&(a, b)| (a.max(0.0), b.min(end_s)))
        .filter(|(a, b)| b > a)
    {
        let mut from = a;
        for &(u, v) in unheard.iter().filter(|&&(u, v)| v > a && u < b) {
            if u > from {
                out.push((from, u));
            }
            from = from.max(v);
        }
        if from < b {
            out.push((from, b));
        }
    }
    out
}

/// How long a watch was heard in the stretches `spans` (start, end; the
/// last may run on) up to `end_s`, seconds: the length of their
/// [`heard_spans`].
pub fn heard_seconds(
    trend: &[TrendReading],
    average_s: f64,
    spans: &[(f64, f64)],
    end_s: f64,
) -> f64 {
    heard_spans(trend, average_s, spans, end_s)
        .iter()
        .map(|(a, b)| b - a)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t: f64, heard: bool) -> TrendReading {
        TrendReading {
            t,
            rate_s_per_day: None,
            amplitude_deg: None,
            amplitude_even_deg: None,
            amplitude_odd_deg: None,
            beat_error_unlock_ms: None,
            beat_error_ms: None,
            rate_error_s_per_day: None,
            amplitude_error_deg: None,
            beat_error_unlock_error_ms: None,
            heard,
        }
    }

    /// The stand, the watch, the stand again (#2177): the stand's noise at
    /// both ends is unheard, the watch's stretch from a reading's look back
    /// before its first heard reading to its last is not.
    #[test]
    fn the_stand_at_both_ends_is_unheard() {
        let trend: Vec<_> = (1..=230)
            .map(|s| at(s as f64, (81..=171).contains(&s)))
            .collect();
        let spans = unheard_spans(&trend, 10.0, 230.0);
        assert_eq!(spans, vec![(0.0, 71.0), (171.0, 230.0)]);
        assert!(in_spans(&spans, 40.0) && in_spans(&spans, 200.0));
        assert!(!in_spans(&spans, 75.0) && !in_spans(&spans, 170.0));
    }

    /// A long take's trend, readings an hour apart: two heard in a row
    /// cover all between them.
    #[test]
    fn heard_readings_far_apart_cover_between() {
        let trend = [at(3600.0, true), at(7200.0, true), at(10800.0, false)];
        let spans = unheard_spans(&trend, 10.0, 10800.0);
        assert_eq!(spans, vec![(0.0, 3590.0), (7200.0, 10800.0)]);
    }

    /// Heard to the end: nothing after the last reading is unheard; with no
    /// readings nothing is known; none heard, all of it is unheard.
    #[test]
    fn the_ends() {
        let heard = [at(10.0, true), at(20.0, true)];
        assert!(unheard_spans(&heard, 10.0, 25.0).is_empty());
        assert!(unheard_spans(&[], 10.0, 25.0).is_empty());
        let none = [at(10.0, false), at(20.0, false)];
        assert_eq!(unheard_spans(&none, 10.0, 25.0), vec![(0.0, 25.0)]);
    }

    /// Silence is not beats (#2183): a take none of whose readings hears
    /// a watch has none heard, however long; one heard from its second
    /// reading on counts from that reading's look back; only the part of
    /// a stretch outside the unheard counts; no reading, nothing known.
    #[test]
    fn heard_seconds_count_the_watch_not_the_sound() {
        let none = [at(10.0, false), at(19.0, false)];
        assert_eq!(heard_seconds(&none, 10.0, &[(0.0, 19.0)], 19.0), 0.0);
        let late = [at(10.0, false), at(20.0, true), at(30.0, true)];
        assert_eq!(heard_seconds(&late, 10.0, &[(0.0, 35.0)], 35.0), 25.0);
        // Two stretches, one in the unheard opening.
        let spans = [(0.0, 8.0), (15.0, 35.0)];
        assert_eq!(heard_seconds(&late, 10.0, &spans, 35.0), 20.0);
        // The last stretch runs on: cut at the end.
        assert_eq!(heard_seconds(&late, 10.0, &[(12.0, 99.0)], 35.0), 23.0);
        assert_eq!(heard_seconds(&[], 10.0, &[(0.0, 35.0)], 35.0), 0.0);
    }

    /// The heard stretches themselves (Norm on #2451): an unheard stretch
    /// inside a span splits it, one over its start or end trims it, and
    /// a span wholly unheard leaves nothing; no reading, none.
    #[test]
    fn heard_spans_leave_the_unheard_out() {
        // Unheard 0..20 (before the first heard reading's look back) and
        // 60..100 (after the reading at 60, until the look back of 110).
        let heard = |k: i32| !(1..=2).contains(&k) && !(7..=10).contains(&k);
        let trend: Vec<_> = (1..=15).map(|k| at(k as f64 * 10.0, heard(k))).collect();
        assert_eq!(
            unheard_spans(&trend, 10.0, 150.0),
            vec![(0.0, 20.0), (60.0, 100.0)]
        );
        let spans = [(5.0, 15.0), (10.0, 130.0), (140.0, 999.0)];
        assert_eq!(
            heard_spans(&trend, 10.0, &spans, 150.0),
            vec![(20.0, 60.0), (100.0, 130.0), (140.0, 150.0)]
        );
        assert_eq!(heard_seconds(&trend, 10.0, &spans, 150.0), 80.0);
        assert!(heard_spans(&[], 10.0, &spans, 150.0).is_empty());
    }
}
