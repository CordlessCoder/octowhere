//! Checks the built-in data against what `tools/tz-data.py` worked out from the same sources.

use octowhere_tz::{DATABASE, DateTime, Rule, civil, local};

#[test]
fn every_rule_parses() {
    assert!(!DATABASE.is_empty());
    for zone in DATABASE.zones() {
        assert!(
            Rule::parse(zone.rule_text()).is_some(),
            "{}: {}",
            zone.name,
            zone.rule_text()
        );
    }
    assert_eq!(DATABASE.tzdata_release(), "2026d");
    assert_eq!(DATABASE.boundary_release(), "2026d");
}

#[test]
fn positions_find_the_generators_zone() {
    let vectors = include_str!("data/points.csv");
    for line in vectors.lines().skip(1) {
        let mut fields = line.split(',');
        let latitude: i32 = fields.next().unwrap().parse().unwrap();
        let longitude: i32 = fields.next().unwrap().parse().unwrap();
        let expected = fields.next().unwrap();
        let found = DATABASE
            .zone_at(latitude, longitude)
            .map_or("", |zone| zone.name);
        assert_eq!(found, expected, "at {latitude}, {longitude}");
    }
}

#[test]
fn offsets_change_where_zoneinfo_changes_them() {
    let vectors = include_str!("data/transitions.csv");
    let mut previous: Option<(&str, i32, bool, &str)> = None;
    for line in vectors.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        let [name, unix, offset, dst, abbreviation] = fields[..] else {
            panic!("{line}");
        };
        let unix: i64 = unix.parse().unwrap();
        let expected = (offset.parse::<i32>().unwrap(), dst == "1", abbreviation);
        let zone = DATABASE.find(name).unwrap();
        let at = zone.at(unix);
        assert_eq!(
            (at.utc_offset, at.dst, at.abbreviation),
            expected,
            "{name} at {unix}"
        );
        if let Some((previous_name, offset, dst, abbreviation)) = previous
            && previous_name == name
        {
            let before = zone.at(unix - 1);
            assert_eq!(
                (before.utc_offset, before.dst, before.abbreviation),
                (offset, dst, abbreviation),
                "{name} just before {unix}"
            );
        }
        previous = Some((name, expected.0, expected.1, expected.2));
    }
}

#[test]
fn dates_round_trip() {
    for days in -800_000..800_000 {
        let (year, month, day) = civil::civil_from_days(days);
        assert_eq!(civil::days_from_civil(year, month, day), days);
        assert!(
            DateTime {
                year,
                month,
                day,
                ..DateTime::default()
            }
            .is_valid()
        );
    }
    assert_eq!(civil::civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil::days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(
        DateTime::from_unix(-1),
        DateTime {
            year: 1969,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59
        }
    );
    // 2026-09-24 was a Thursday.
    assert_eq!(
        DateTime {
            year: 2026,
            month: 9,
            day: 24,
            ..DateTime::default()
        }
        .weekday(),
        4
    );
}

#[test]
fn dublin_is_ahead_in_summer() {
    let zone = DATABASE.find("Europe/Dublin").unwrap();
    let noon_utc = DateTime {
        year: 2026,
        month: 7,
        day: 1,
        hour: 12,
        ..DateTime::default()
    };
    let (time, offset) = local(noon_utc.to_unix(), &zone);
    assert_eq!(
        (time.hour, offset.abbreviation, offset.utc_offset),
        (13, "IST", 3600)
    );
}

#[test]
fn rules_in_every_form_parse() {
    for (text, summer, winter) in [
        // Julian days never count 29 February; plain days do.
        ("AAA3BBB,J60/2,J300/2", -2 * 3600, -3 * 3600),
        ("AAA3BBB,59/2,299/2", -2 * 3600, -3 * 3600),
        ("<-03>3<-02>,M3.5.0/-2,M10.5.0/-1", -2 * 3600, -3 * 3600),
        ("<+0530>-5:30", 19_800, 19_800),
        ("AAA-10BBB-11:00:00,M10.1.0,M4.1.0/3", 36_000, 39_600),
    ] {
        let rule = Rule::parse(text).unwrap_or_else(|| panic!("{text}"));
        let july = DateTime {
            year: 2027,
            month: 7,
            day: 1,
            ..DateTime::default()
        }
        .to_unix();
        let january = DateTime {
            year: 2027,
            month: 1,
            day: 15,
            ..DateTime::default()
        }
        .to_unix();
        assert_eq!(
            (rule.at(july).utc_offset, rule.at(january).utc_offset),
            (summer, winter),
            "{text}"
        );
    }
    for bad in [
        "",
        "AB1",
        "AAA",
        "AAA1BBB,M13.1.0,M1.1.0",
        "AAA1BBB,M3.2.0",
        "AAA25",
        "<AAA1",
    ] {
        assert!(Rule::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn zones_have_reference_points_except_etc() {
    let dublin = octowhere_tz::DATABASE.find("Europe/Dublin").unwrap();
    assert_eq!(dublin.reference(), Some((533_300_000, -62_500_000)));
    assert_eq!(octowhere_tz::DATABASE.find("Etc/GMT-1").unwrap().reference(), None);
    let without = octowhere_tz::DATABASE
        .zones()
        .filter(|zone| !zone.name.starts_with("Etc/") && zone.reference().is_none())
        .count();
    assert!(without < 40, "{without} zones have no reference point");
}
