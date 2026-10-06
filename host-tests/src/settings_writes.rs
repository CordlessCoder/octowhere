use octowhere_ui::{
    tz::DATABASE,
    ui::{clock::ZoneId, rest::Timeout},
};

use crate::settings_queue::{SettingsQueue, Write};

fn zone(name: &str) -> ZoneId {
    DATABASE.find(name).unwrap().id
}

fn drained(queue: &mut SettingsQueue) -> Vec<Write> {
    core::iter::from_fn(|| queue.pop()).collect()
}

#[test]
fn a_later_change_of_a_setting_replaces_the_one_waiting() {
    let mut queue = SettingsQueue::new();
    for level in 0..20 {
        queue.push(Write::Brightness(level));
    }
    queue.push(Write::Timeout(Timeout::default()));
    assert_eq!(
        drained(&mut queue),
        [Write::Brightness(19), Write::Timeout(Timeout::default())]
    );
    assert!(queue.is_empty());
}

#[test]
fn a_clear_is_kept_and_goes_before_what_follows_it() {
    let mut queue = SettingsQueue::new();
    queue.push(Write::Brightness(80));
    queue.push(Write::ManualZone(zone("Europe/Dublin")));
    queue.push(Write::Clear);
    for level in 0..20 {
        queue.push(Write::Brightness(level));
    }
    assert_eq!(drained(&mut queue), [Write::Clear, Write::Brightness(19)]);
}

#[test]
fn the_zone_mode_ends_as_it_was_last_chosen() {
    let dublin = zone("Europe/Dublin");
    let mut queue = SettingsQueue::new();
    queue.push(Write::ManualZone(dublin));
    queue.push(Write::Automatic);
    assert_eq!(
        drained(&mut queue),
        [Write::ManualZone(dublin), Write::Automatic]
    );
    queue.push(Write::Automatic);
    queue.push(Write::ManualZone(dublin));
    assert_eq!(drained(&mut queue), [Write::ManualZone(dublin)]);
}
