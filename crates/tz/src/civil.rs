//! Proleptic Gregorian dates and Unix time, after Howard Hinnant's `days_from_civil`.

pub const SECONDS_PER_DAY: i64 = 86_400;

/// A date and time of day, with no zone attached.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct DateTime {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    /// The date and time `seconds` after 1970-01-01 00:00:00.
    #[must_use]
    pub const fn from_unix(seconds: i64) -> Self {
        let days = seconds.div_euclid(SECONDS_PER_DAY);
        let of_day = seconds.rem_euclid(SECONDS_PER_DAY) as u32;
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: (of_day / 3600) as u8,
            minute: (of_day / 60 % 60) as u8,
            second: (of_day % 60) as u8,
        }
    }

    /// Seconds since 1970-01-01 00:00:00, taking this as that clock's time.
    #[must_use]
    pub const fn to_unix(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * SECONDS_PER_DAY
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
            + self.second as i64
    }

    /// 0 for Sunday to 6 for Saturday.
    #[must_use]
    pub const fn weekday(&self) -> u8 {
        weekday(days_from_civil(self.year, self.month, self.day))
    }

    /// Whether the fields name a real date and time of day.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.month >= 1
            && self.month <= 12
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
            && self.hour < 24
            && self.minute < 60
            && self.second < 60
    }
}

#[must_use]
pub const fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[must_use]
pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to the given date.
#[must_use]
pub const fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = year as i64 - (month <= 2) as i64;
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let month = month as i64;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_era_days - 719_468
}

/// The date `days` after 1970-01-01.
#[must_use]
pub const fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let of_era = days - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = (of_year - (153 * shifted_month + 2) / 5 + 1) as u8;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u8;
    let year = year_of_era + era * 400 + (month <= 2) as i64;
    (year as i32, month, day)
}

/// 0 for Sunday to 6 for Saturday, for the day `days` after 1970-01-01, a Thursday.
#[must_use]
pub const fn weekday(days: i64) -> u8 {
    (days + 4).rem_euclid(7) as u8
}
