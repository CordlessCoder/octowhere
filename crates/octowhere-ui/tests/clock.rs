use octowhere_ui::{
    tz::DATABASE,
    ui::clock::{ClockState, DateTime, ZoneMode, ZoneState},
};

fn clock(time: DateTime) -> ClockState {
    ClockState {
        utc: Some(time.to_unix()),
        ..ClockState::default()
    }
}

#[test]
fn local_time_needs_both_a_reading_and_a_zone() {
    let noon = DateTime { year: 2026, month: 1, day: 10, hour: 12, ..DateTime::default() };
    let kolkata = ZoneState {
        mode: ZoneMode::Manual,
        zone: DATABASE.find("Asia/Kolkata").map(|zone| zone.id),
    };
    let local = clock(noon).local(kolkata).expect("a reading and a zone");
    assert_eq!((local.time.hour, local.time.minute), (17, 30));
    assert_eq!((local.offset.abbreviation, local.zone), ("IST", "Asia/Kolkata"));
    assert_eq!(clock(noon).local(ZoneState::default()), None);
    assert_eq!(ClockState::default().local(kolkata), None);
}
