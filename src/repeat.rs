use crate::db::{RepeatRule, RepeatUnit};
use gtk4::glib::{self, DateTime};

pub fn next_due_at(
    rule: &RepeatRule,
    due_at: &str,
    now: &DateTime,
) -> Result<Option<String>, glib::BoolError> {
    match rule {
        RepeatRule::None => return Ok(None),
        RepeatRule::Interval { every, .. } if !(1..=365).contains(every) => {
            return Err(glib::bool_error!(
                "Repeat interval must be between 1 and 365"
            ));
        }
        RepeatRule::Weekdays { days } if !(1..=127).contains(days) => {
            return Err(glib::bool_error!("Choose at least one valid weekday"));
        }
        _ => {}
    }
    let anchor = DateTime::from_iso8601(due_at, None)?.to_local()?;
    let now = now.to_local()?;
    let next = match rule {
        RepeatRule::None => return Ok(None),
        RepeatRule::Interval { every, unit } => {
            let every = *every as i32;
            match unit {
                RepeatUnit::Minutes | RepeatUnit::Hours => {
                    let seconds = i64::from(every)
                        * if *unit == RepeatUnit::Minutes {
                            60
                        } else {
                            3600
                        };
                    let elapsed = now.to_unix() - anchor.to_unix();
                    let count = if elapsed < 0 {
                        1
                    } else {
                        elapsed / seconds + 1
                    };
                    DateTime::from_unix_utc(anchor.to_unix() + count * seconds)?.to_local()?
                }
                RepeatUnit::Days | RepeatUnit::Weeks => {
                    let mut candidate = if *unit == RepeatUnit::Days {
                        anchor.add_days(every)?
                    } else {
                        anchor.add_weeks(every)?
                    };
                    while candidate.to_unix() <= now.to_unix() {
                        candidate = if *unit == RepeatUnit::Days {
                            candidate.add_days(every)?
                        } else {
                            candidate.add_weeks(every)?
                        };
                    }
                    candidate
                }
            }
        }
        RepeatRule::Weekdays { days } => {
            let mut candidate = anchor.add_days(1)?;
            while candidate.to_unix() <= now.to_unix()
                || days & (1 << (candidate.day_of_week() - 1)) == 0
            {
                candidate = candidate.add_days(1)?;
            }
            candidate
        }
    };
    Ok(Some(
        next.to_utc()?.format("%Y-%m-%dT%H:%M:%SZ")?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_skips_missed_occurrences_and_keeps_clock_time() {
        let due = DateTime::from_local(2026, 11, 9, 9, 0, 0.0).unwrap();
        let now = DateTime::from_local(2026, 11, 9, 12, 15, 0.0).unwrap();
        let due_utc = due.to_utc().unwrap().format("%Y-%m-%dT%H:%M:%SZ").unwrap();
        let next = next_due_at(
            &RepeatRule::Interval {
                every: 2,
                unit: RepeatUnit::Hours,
            },
            &due_utc,
            &now,
        )
        .unwrap()
        .unwrap();
        let next_local = DateTime::from_iso8601(&next, None)
            .unwrap()
            .to_local()
            .unwrap();
        assert_eq!((next_local.day_of_month(), next_local.hour()), (9, 13));
    }

    #[test]
    fn weekday_repeat_uses_selected_local_day_and_time() {
        let due = DateTime::from_local(2026, 11, 9, 9, 0, 0.0).unwrap();
        let now = DateTime::from_local(2026, 11, 10, 10, 0, 0.0).unwrap();
        let due_utc = due.to_utc().unwrap().format("%Y-%m-%dT%H:%M:%SZ").unwrap();
        let next = next_due_at(&RepeatRule::Weekdays { days: 1 << 4 }, &due_utc, &now)
            .unwrap()
            .unwrap();
        let next_local = DateTime::from_iso8601(&next, None)
            .unwrap()
            .to_local()
            .unwrap();
        assert_eq!(
            (
                next_local.year(),
                next_local.month(),
                next_local.day_of_month(),
                next_local.hour()
            ),
            (2026, 11, 13, 9)
        );
    }
}
