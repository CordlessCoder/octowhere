//! A zone's current rule, as the POSIX `TZ` string that closes its TZif file (RFC 9636 §3.3).

use crate::civil::{self, SECONDS_PER_DAY};

/// What a zone's clocks read at some instant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Offset<'a> {
    /// Seconds to add to UTC, east positive.
    pub utc_offset: i32,
    pub dst: bool,
    /// Such as `CEST`, or `+0530` where the zone has no letters.
    pub abbreviation: &'a str,
}

/// A zone's standard time, and its daylight saving time if it keeps one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rule<'a> {
    standard: Time<'a>,
    daylight: Option<Daylight<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Time<'a> {
    abbreviation: &'a str,
    utc_offset: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Daylight<'a> {
    time: Time<'a>,
    /// Given in standard time.
    start: Change,
    /// Given in daylight saving time.
    end: Change,
}

/// When in a year the clocks change: a day, and seconds into it on the clock then in force.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Change {
    day: Day,
    seconds: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Day {
    /// `Jn`: 1 to 365, never counting 29 February.
    Julian(u16),
    /// `n`: 0 to 365, counting 29 February.
    Ordinal(u16),
    /// `Mm.w.d`: weekday `d` (0 Sunday) of week `w` (5 the last) of month `m`.
    Month { month: u8, week: u8, weekday: u8 },
}

impl<'a> Rule<'a> {
    /// Parses a rule, or `None` if `text` is not one.
    #[must_use]
    pub fn parse(text: &'a str) -> Option<Self> {
        let mut parser = Parser { rest: text };
        let standard = Time {
            abbreviation: parser.abbreviation()?,
            utc_offset: -parser.duration(24)?,
        };
        if parser.rest.is_empty() {
            return Some(Self {
                standard,
                daylight: None,
            });
        }
        let abbreviation = parser.abbreviation()?;
        let utc_offset = if parser.rest.is_empty() || parser.rest.starts_with(',') {
            standard.utc_offset + 3600
        } else {
            -parser.duration(24)?
        };
        let (start, end) = if parser.rest.is_empty() {
            // The US rules, as the tz reference code assumes when none are given.
            (
                Change {
                    day: Day::Month {
                        month: 3,
                        week: 2,
                        weekday: 0,
                    },
                    seconds: 7200,
                },
                Change {
                    day: Day::Month {
                        month: 11,
                        week: 1,
                        weekday: 0,
                    },
                    seconds: 7200,
                },
            )
        } else {
            parser.expect(',')?;
            let start = parser.change()?;
            parser.expect(',')?;
            (start, parser.change()?)
        };
        parser.rest.is_empty().then_some(Self {
            standard,
            daylight: Some(Daylight {
                time: Time {
                    abbreviation,
                    utc_offset,
                },
                start,
                end,
            }),
        })
    }

    /// What the clocks read at `unix`, in seconds since 1970-01-01 UTC.
    #[must_use]
    pub fn at(&self, unix: i64) -> Offset<'a> {
        let Some(daylight) = self.daylight else {
            return self.standard.offset(false);
        };
        let (year, _, _) = civil::civil_from_days(
            (unix + i64::from(self.standard.utc_offset)).div_euclid(SECONDS_PER_DAY),
        );
        // The changes of the years either side as well, since a change can fall outside its own
        // year once its time of day passes midnight or the offsets move it. At equal instants a
        // start sorts after an end, so a zone in daylight saving all year stays in it.
        let mut changes = [(0, false); 6];
        for (slot, year) in (year - 1..=year + 1).enumerate() {
            changes[2 * slot] = (daylight.end.instant(year, daylight.time.utc_offset), false);
            changes[2 * slot + 1] = (daylight.start.instant(year, self.standard.utc_offset), true);
        }
        changes.sort_unstable();
        let dst = changes
            .iter()
            .rev()
            .find(|&&(instant, _)| instant <= unix)
            .is_some_and(|&(_, starts)| starts);
        if dst {
            daylight.time.offset(true)
        } else {
            self.standard.offset(false)
        }
    }

    /// Whether the zone keeps daylight saving time at all.
    #[must_use]
    pub fn has_daylight_saving(&self) -> bool {
        self.daylight.is_some()
    }
}

impl<'a> Time<'a> {
    fn offset(self, dst: bool) -> Offset<'a> {
        Offset {
            utc_offset: self.utc_offset,
            dst,
            abbreviation: self.abbreviation,
        }
    }
}

impl Change {
    /// The change in `year`, in Unix seconds, where the clocks read `utc_offset` before it.
    fn instant(self, year: i32, utc_offset: i32) -> i64 {
        let days = match self.day {
            Day::Julian(day) => {
                let skip_leap_day = civil::is_leap(year) && day >= 60;
                civil::days_from_civil(year, 1, 1) + i64::from(day) - 1 + i64::from(skip_leap_day)
            }
            Day::Ordinal(day) => civil::days_from_civil(year, 1, 1) + i64::from(day),
            Day::Month {
                month,
                week,
                weekday,
            } => {
                let first = civil::days_from_civil(year, month, 1);
                let mut day = first + i64::from((weekday + 7 - civil::weekday(first)) % 7);
                day += 7 * i64::from(week - 1);
                let last = first + i64::from(civil::days_in_month(year, month)) - 1;
                while day > last {
                    day -= 7;
                }
                day
            }
        };
        days * SECONDS_PER_DAY + i64::from(self.seconds) - i64::from(utc_offset)
    }
}

struct Parser<'a> {
    rest: &'a str,
}

impl<'a> Parser<'a> {
    fn expect(&mut self, c: char) -> Option<()> {
        self.rest = self.rest.strip_prefix(c)?;
        Some(())
    }

    /// Either letters, or anything but `>` between `<` and `>`.
    fn abbreviation(&mut self) -> Option<&'a str> {
        let (name, rest) = if let Some(quoted) = self.rest.strip_prefix('<') {
            quoted.split_once('>')?
        } else {
            let end = self
                .rest
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(self.rest.len());
            self.rest.split_at(end)
        };
        self.rest = rest;
        (name.len() >= 3).then_some(name)
    }

    fn number(&mut self) -> Option<u32> {
        let end = self
            .rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(self.rest.len());
        let (digits, rest) = self.rest.split_at(end);
        self.rest = rest;
        digits.parse().ok()
    }

    /// `[+-]hh[:mm[:ss]]` in seconds, with the hours at most `max_hours`.
    fn duration(&mut self, max_hours: u32) -> Option<i32> {
        let negative = self.rest.starts_with('-');
        if negative || self.rest.starts_with('+') {
            self.rest = &self.rest[1..];
        }
        let hours = self.number()?;
        let mut seconds = hours * 3600;
        for scale in [60, 1] {
            if self.expect(':').is_none() {
                break;
            }
            let part = self.number()?;
            if part > 59 {
                return None;
            }
            seconds += part * scale;
        }
        if hours > max_hours {
            return None;
        }
        let seconds = seconds as i32;
        Some(if negative { -seconds } else { seconds })
    }

    fn change(&mut self) -> Option<Change> {
        let day = if self.expect('J').is_some() {
            Day::Julian(self.number().filter(|day| (1..=365).contains(day))? as u16)
        } else if self.expect('M').is_some() {
            let month = self.number().filter(|month| (1..=12).contains(month))? as u8;
            self.expect('.')?;
            let week = self.number().filter(|week| (1..=5).contains(week))? as u8;
            self.expect('.')?;
            let weekday = self.number().filter(|day| *day <= 6)? as u8;
            Day::Month {
                month,
                week,
                weekday,
            }
        } else {
            Day::Ordinal(self.number().filter(|day| *day <= 365)? as u16)
        };
        let seconds = if self.expect('/').is_some() {
            self.duration(167)?
        } else {
            7200
        };
        Some(Change { day, seconds })
    }
}
