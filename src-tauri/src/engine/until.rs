//! The "Next occurrence of HH:MM" resolver used by the Sessions page timer.
//!
//! This is deliberately generic over `TimeZone` and its gap/overlap handling is
//! factored out into [`core_resolve`] so the daylight-saving edges can be tested
//! with a fake offset function instead of depending on the machine's real zone.

use chrono::{DateTime, Duration, FixedOffset, LocalResult, NaiveDateTime, Offset, TimeZone};
use serde::Serialize;

/// What an "Until HH:MM" timer resolves to.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct UntilTarget {
    pub seconds: i64,
    pub tomorrow: bool,
    pub label: String,
}

/// Resolve the next local `hour:minute` strictly after `now`.
pub fn resolve_until<Tz: TimeZone>(
    tz: &Tz,
    now: &DateTime<Tz>,
    hour: u32,
    minute: u32,
) -> UntilTarget {
    let offset_of = |naive: NaiveDateTime| -> LocalResult<FixedOffset> {
        tz.offset_from_local_datetime(&naive).map(|o| o.fix())
    };
    let (seconds, tomorrow) =
        core_resolve(offset_of, now.naive_local(), now.naive_utc(), hour, minute);
    UntilTarget {
        seconds,
        tomorrow,
        label: label_for(hour, minute, tomorrow),
    }
}

/// `Today 14:30` / `Tomorrow 02:30`, matching the Sessions page wording.
pub fn label_for(hour: u32, minute: u32, tomorrow: bool) -> String {
    let day = if tomorrow { "Tomorrow" } else { "Today" };
    format!("{day} {:02}:{:02}", hour.min(23), minute.min(59))
}

/// The pure core. `utc_now` is the current instant in UTC; `local_now` is the
/// same instant as wall-clock time; `offset_of` answers "what offset does this
/// wall-clock time have?" (`None` inside a spring-forward gap).
pub fn core_resolve<F>(
    offset_of: F,
    local_now: NaiveDateTime,
    utc_now: NaiveDateTime,
    hour: u32,
    minute: u32,
) -> (i64, bool)
where
    F: Fn(NaiveDateTime) -> LocalResult<FixedOffset>,
{
    let hour = hour.min(23);
    let minute = minute.min(59);
    let mut day = local_now.date();
    let mut tomorrow = false;

    // Today, then tomorrow, then a plain 24 hours as an absolute fallback.
    for _ in 0..2 {
        if let Some(naive) = day.and_hms_opt(hour, minute, 0)
            && let Some(instant) = instant_of(&offset_of, naive)
            && instant > utc_now
        {
            let seconds = (instant - utc_now).num_seconds().max(0);
            return (seconds, tomorrow);
        }
        tomorrow = true;
        day = match day.succ_opt() {
            Some(next) => next,
            None => return (86_400, true),
        };
    }
    (86_400, true)
}

/// Turn a wall-clock time into a UTC instant, stepping over a spring-forward gap
/// (a few minutes at a time, bounded) and taking the first occurrence of an
/// ambiguous fall-back time.
fn instant_of<F>(offset_of: &F, mut naive: NaiveDateTime) -> Option<NaiveDateTime>
where
    F: Fn(NaiveDateTime) -> LocalResult<FixedOffset>,
{
    for _ in 0..241 {
        match offset_of(naive) {
            LocalResult::Single(off) => {
                return off
                    .from_local_datetime(&naive)
                    .single()
                    .map(|d| d.naive_utc());
            }
            LocalResult::Ambiguous(first, second) => {
                // The earlier *instant* has the larger UTC offset.
                let off = if first.local_minus_utc() >= second.local_minus_utc() {
                    first
                } else {
                    second
                };
                return off
                    .from_local_datetime(&naive)
                    .single()
                    .map(|d| d.naive_utc());
            }
            LocalResult::None => naive += Duration::minutes(1),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    fn off(seconds: i32) -> FixedOffset {
        FixedOffset::east_opt(seconds).unwrap()
    }

    /// Spring forward: 2026-03-08 02:00 local -> 03:00. 02:00..02:59 does not
    /// exist. Offset moves from -6h to -5h.
    fn spring_forward(naive: NaiveDateTime) -> LocalResult<FixedOffset> {
        let gap_start = dt(2026, 3, 8, 2, 0);
        let gap_end = dt(2026, 3, 8, 3, 0);
        if naive >= gap_start && naive < gap_end {
            return LocalResult::None;
        }
        if naive >= gap_end {
            LocalResult::Single(off(-5 * 3600))
        } else {
            LocalResult::Single(off(-6 * 3600))
        }
    }

    /// Fall back: 2026-11-01 01:00..01:59 happens twice. Offset moves from -5h
    /// to -6h.
    fn fall_back(naive: NaiveDateTime) -> LocalResult<FixedOffset> {
        let first = dt(2026, 11, 1, 1, 0);
        let second = dt(2026, 11, 1, 2, 0);
        if naive < first {
            LocalResult::Single(off(-5 * 3600))
        } else if naive < second {
            LocalResult::Ambiguous(off(-5 * 3600), off(-6 * 3600))
        } else {
            LocalResult::Single(off(-6 * 3600))
        }
    }

    #[test]
    fn resolves_to_today_when_still_ahead() {
        // 2026-03-07 01:00 local (-6h) => 07:00 UTC.
        let (seconds, tomorrow) = core_resolve(
            spring_forward,
            dt(2026, 3, 7, 1, 0),
            dt(2026, 3, 7, 7, 0),
            2,
            30,
        );
        assert!(!tomorrow);
        assert_eq!(seconds, 5400, "01:00 -> 02:30 same day is 1.5h");
    }

    #[test]
    fn resolves_to_tomorrow_when_already_past() {
        // 2026-03-07 10:00 local (-6h) => 16:00 UTC; target already gone today.
        let (seconds, tomorrow) = core_resolve(
            spring_forward,
            dt(2026, 3, 7, 10, 0),
            dt(2026, 3, 7, 16, 0),
            9,
            0,
        );
        assert!(tomorrow);
        assert_eq!(
            seconds,
            22 * 3600,
            "10:00 -> tomorrow 09:00 is 22h, not 23h: the spring-forward switch \
             loses an hour the next morning"
        );
    }

    #[test]
    fn spring_forward_gap_shifts_the_target_and_shortens_the_wait() {
        // Now = 2026-03-07 02:30 local (-6h) => 08:30 UTC. Target 02:30 is
        // exactly "now" today, so it rolls to tomorrow 02:30 — which does not
        // exist and slides to 03:00 local (-5h) => 08:00 UTC on 03-08.
        let (seconds, tomorrow) = core_resolve(
            spring_forward,
            dt(2026, 3, 7, 2, 30),
            dt(2026, 3, 7, 8, 30),
            2,
            30,
        );
        assert!(tomorrow);
        assert_eq!(
            seconds, 84_600,
            "23.5h, not 24h: the lost 30 minutes come from the DST gap"
        );
        assert_ne!(seconds, 86_400);

        // A naive "add 24 hours" resolver would have said 86_400 here.
        assert!(seconds < 86_400);
    }

    #[test]
    fn spring_forward_day_still_resolves_later_times_normally() {
        // Target 12:00 on the DST day is untouched by the gap.
        let (seconds, tomorrow) = core_resolve(
            spring_forward,
            dt(2026, 3, 7, 12, 0),
            dt(2026, 3, 7, 18, 0),
            12,
            0,
        );
        assert!(tomorrow);
        assert_eq!(seconds, 23 * 3600);
    }

    #[test]
    fn fall_back_ambiguous_time_uses_the_first_occurrence() {
        // Now = 2026-11-01 00:30 local (-5h) => 05:30 UTC. Target 01:30 is
        // ambiguous; the first pass is -5h => 06:30 UTC.
        let (seconds, tomorrow) = core_resolve(
            fall_back,
            dt(2026, 11, 1, 0, 30),
            dt(2026, 11, 1, 5, 30),
            1,
            30,
        );
        assert!(!tomorrow);
        assert_eq!(seconds, 3600, "first pass of an ambiguous hour is 1h away");
    }

    #[test]
    fn fall_back_after_the_repeat_targets_the_next_day() {
        // 2026-11-01 01:45 local (-5h => 06:45 UTC) with target 01:30: the
        // remaining occurrences are in the past, so roll to 11-02.
        let (seconds, tomorrow) = core_resolve(
            fall_back,
            dt(2026, 11, 1, 1, 45),
            dt(2026, 11, 1, 6, 45),
            1,
            30,
        );
        assert!(tomorrow);
        // 11-02 01:30 is at -6h => 07:30 UTC, which is 24h45m after 06:45 UTC.
        assert_eq!(seconds, 24 * 3600 + 45 * 60);
    }

    #[test]
    fn midnight_target_rolls_forward_a_whole_day() {
        let flat = |_: NaiveDateTime| LocalResult::Single(off(0));
        let (seconds, tomorrow) =
            core_resolve(flat, dt(2026, 6, 1, 0, 0), dt(2026, 6, 1, 0, 0), 0, 0);
        assert!(tomorrow, "exactly now is not in the future");
        assert_eq!(seconds, 86_400);
        let (seconds, tomorrow) =
            core_resolve(flat, dt(2026, 6, 1, 23, 59), dt(2026, 6, 1, 23, 59), 0, 0);
        assert!(tomorrow);
        assert_eq!(seconds, 60);
    }

    #[test]
    fn labels_read_naturally() {
        assert_eq!(label_for(14, 30, false), "Today 14:30");
        assert_eq!(label_for(2, 5, true), "Tomorrow 02:05");
        assert_eq!(label_for(99, 99, true), "Tomorrow 23:59");
    }

    #[test]
    fn real_local_zone_always_returns_a_positive_duration() {
        use chrono::Local;
        let now = Local::now();
        let target = resolve_until(&Local, &now, 3, 17);
        assert!(target.seconds > 0);
        assert!(target.seconds <= 2 * 86_400);
    }
}
