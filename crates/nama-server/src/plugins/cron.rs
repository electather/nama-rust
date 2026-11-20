//! A Job's schedule: a standard 5-field cron expression, read to the minute (ADR 0079).

use jiff::{ToSpan as _, civil::DateTime};

/// A parsed schedule: for each field, a bit per value it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Cron {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    /// Whether day of month or day of week is `*`: then a day must match both, as in Vixie
    /// cron, and otherwise either.
    any_day: bool,
}

/// One of a schedule's 5 fields: what it is called, the values it takes and their names.
struct Field {
    name: &'static str,
    min: u8,
    max: u8,
    /// The names of `min` onwards, such as `jan` for month 1.
    names: &'static [&'static str],
}

const FIELDS: [Field; 5] = [
    Field {
        name: "minute",
        min: 0,
        max: 59,
        names: &[],
    },
    Field {
        name: "hour",
        min: 0,
        max: 23,
        names: &[],
    },
    Field {
        name: "day of month",
        min: 1,
        max: 31,
        names: &[],
    },
    Field {
        name: "month",
        min: 1,
        max: 12,
        names: &[
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ],
    },
    Field {
        name: "day of week",
        min: 0,
        // 7 is Sunday again.
        max: 7,
        names: &["sun", "mon", "tue", "wed", "thu", "fri", "sat"],
    },
];

impl Cron {
    /// Reads `text`, such as `0 4 * * *`.
    ///
    /// # Errors
    ///
    /// Why it isn't a schedule, in plain words that quote it, such as a seconds field.
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields[..] else {
            return Err(match fields.len() {
                6 => format!(
                    "{text:?} has a seconds field; a schedule has 5 fields and runs to the minute"
                ),
                count => format!(
                    "{text:?} has {count} field{}; a schedule has 5: minute, hour, day of month, \
                     month and day of week",
                    if count == 1 { "" } else { "s" }
                ),
            });
        };
        let [minutes, hours, days, months, weekdays] = &FIELDS;
        let quoted = |field: &Field, value: &str| {
            bits(field, value).map_err(|why| format!("{text:?} has {why}"))
        };
        let weekdays = quoted(weekdays, weekday)?;
        Ok(Self {
            minutes: quoted(minutes, minute)?,
            hours: quoted(hours, hour)?,
            days: quoted(days, day)?,
            months: quoted(months, month)?,
            weekdays: (weekdays | weekdays >> 7) & 0x7f,
            any_day: day.starts_with('*') || weekday.starts_with('*'),
        })
    }

    /// Whether the schedule names a minute starting after `after` and no later than `until`, both
    /// wall-clock times. So a minute is fired once however the windows asked about meet, and a
    /// minute before `after`, such as one missed while the Server was down, never. A minute the
    /// clocks skip falls in the window they skip it in, so it fires then, as in Vixie cron.
    pub(super) fn fires_between(&self, after: DateTime, until: DateTime) -> bool {
        let Ok(first) = after.with().second(0).subsec_nanosecond(0).build() else {
            return false;
        };
        (first.series(1.minute()))
            .skip(1)
            .take_while(|minute| *minute <= until)
            .any(|minute| self.matches(minute))
    }

    /// Whether the schedule names the minute `at` is in.
    pub(super) fn matches(&self, at: DateTime) -> bool {
        let has = |bits: u64, value: i8| bits >> value & 1 == 1;
        let day = has(self.days, at.day());
        let weekday = has(self.weekdays, at.weekday().to_sunday_zero_offset());
        let day_matches = if self.any_day {
            day && weekday
        } else {
            day || weekday
        };
        has(self.minutes, at.minute())
            && has(self.hours, at.hour())
            && has(self.months, at.month())
            && day_matches
    }
}

/// A bit per value of `field` that `text`, such as `*/15` or `1-5,7`, names, or why it can't.
fn bits(field: &Field, text: &str) -> Result<u64, String> {
    text.split(',').try_fold(0, |bits, item| {
        let (range, step) = match item.split_once('/') {
            Some((range, step)) => (range, Some(step)),
            None => (item, None),
        };
        let (low, high) = match range.split_once('-') {
            _ if range == "*" => (field.min, field.max),
            Some((low, high)) => (value(field, low)?, value(field, high)?),
            // `5/10` is 5, 15, 25 and on, as in Vixie cron.
            None if step.is_some() => (value(field, range)?, field.max),
            None => {
                let value = value(field, range)?;
                (value, value)
            }
        };
        if low > high {
            return Err(format!("a range from {low} down to {high}"));
        }
        let step = match step.map(str::parse::<u8>) {
            None => 1,
            Some(Ok(step)) if step > 0 => step,
            Some(_) => return Err(format!("a step of {}", step.unwrap_or_default())),
        };
        Ok((low..=high)
            .step_by(step.into())
            .fold(bits, |bits, value| bits | 1 << value))
    })
}

/// The value `text` names in `field`, a number or a name, or why it isn't one.
fn value(field: &Field, text: &str) -> Result<u8, String> {
    let named = field
        .names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text))
        .and_then(|index| u8::try_from(index).ok())
        .map(|index| index + field.min);
    let value = named
        .or_else(|| text.parse().ok())
        .ok_or_else(|| format!("{} {text}, which isn't one", field.name))?;
    if !(field.min..=field.max).contains(&value) {
        return Err(format!(
            "{} {value}, outside {}-{}",
            field.name, field.min, field.max
        ));
    }
    Ok(value)
}

#[cfg(test)]
#[path = "cron_tests.rs"]
mod tests;
