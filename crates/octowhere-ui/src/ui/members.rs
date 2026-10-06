//! The member face: where the group's members are, as bearings round a ring that turns with
//! the device's true heading over a grid that keeps to true north, with the selected member's
//! distance and ages across the middle (2026-10-04 hand-off). Without a fix of its own, or
//! without members' positions, the face keeps its grid and ring and says what it lacks.
//!
//! The ring is not a map: every member sits on it at the same radius, whatever its distance.
//! Members whose rim labels would overlap are drawn as a sector, an arc over the bearings they
//! span with their count and their ages' range, never as a member at their mean (2026-10-05
//! hand-off). The selected member keeps its own node at its own bearing; a tap in the middle
//! steps the selection through every member with a position, so each can be shown on its own.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::Vec;
use octowhere_mesh::packet::MAX_DELTA;

use super::{
    clock::ClockState,
    compass::CompassView,
    drawer::{age, age_due},
    gesture::Micros,
    group::{
        layout::{
            Align, Arc, Backdrop, Face, Line, List, Shape, Text, Turned, Vertical, format, rect,
        },
        view::{GroupView, IDS, Position},
    },
    screens::Gnss,
    stroke,
    text::style,
};
use crate::chrome::{self, Color, CoverageTarget, FontdueRenderer};

const SECOND: Micros = 1_000_000;
const CX: f32 = 233.0;
const CY: f32 = 233.0;
const CENTER: Point = Point::new(233, 233);
/// The bearing ring, to the middle of its stroke, and where nodes, rim labels and the north
/// mark are centred.
const RING: f32 = 212.5;
const NODE_RADIUS: f32 = 213.0;
const LABEL_RADIUS: f32 = 188.0;
const NORTH_RADIUS: f32 = 226.0;
/// Half a node frame's side.
const NODE_HALF: i32 = 12;
/// How far apart two rim labels keep their ink along the label ring.
const LABEL_GAP: f32 = 6.0;
/// Half a rim label's ink height, which brings its inner corners nearer the centre.
const LABEL_HALF_HEIGHT: f32 = 5.0;
/// A sector's arc, how far its end marks reach either side of it, and where its summary is
/// centred.
const SECTOR_RADIUS: f32 = 205.0;
const SECTOR_MARK: f32 = 4.0;
const SUMMARY_RADIUS: f32 = 174.0;
/// The caption under the middle that sums up the rest of the selected member's sector.
const SECTOR_TOP: i32 = 372;
/// A whole ring, a little over a turn so that it has no seam.
const WHOLE: (i16, i16) = (-900, 2720);
/// The grid's pitch, which is a texture and not a scale, and how far out it is drawn: the
/// glass and as far past it as the pixel shift reaches.
const PITCH: i32 = 64;
const GRID_REACH: f32 = 236.0;
/// The grid's lines, three quarters of a pixel wide, and its marks' side.
const GRID_LINE_WIDTH: u8 = 3;
const GRID_MARK: u32 = 2;
/// A position younger than this shows the solid glyph and an older one the hourglass. The
/// hand-off proposes it and leaves it to engineering; explicit ages show either way.
const RECENT: Micros = 5 * 60 * SECOND;
/// A position older than this has left the protocol's table, so the face no longer places it.
pub const EXPIRES: Micros = MAX_DELTA as Micros * SECOND;
/// The forward arrow, in quarter pixels.
const ARROW: [(i16, i16); 4] = [(932, 856), (900, 988), (932, 968), (964, 988)];
/// The middle a tap selects the next member in, inside the rim labels.
const MIDDLE_RADIUS: i32 = 160;
/// The box with the selected member's coordinates while this device has no fix.
const COORDINATES: Rectangle = rect(78, 248, 388, 360);
/// The control that opens the members or the group while no member is placed.
const BUTTON: Rectangle = rect(94, 305, 372, 389);
/// The selected member's name: its widest ink in the left column, and the sizes it may take.
const NAME_WIDTH: u32 = 106;
const NAME_SIZES: (u8, u8) = (25, 14);
/// The distance's widest ink and sizes.
const DISTANCE_WIDTH: u32 = 100;
const DISTANCE_SIZES: (u8, u8) = (39, 26);

/// What the face is drawn from.
pub struct Context<'a> {
    pub group: Option<&'a GroupView>,
    pub gnss: &'a Gnss,
    /// Whole degrees clockwise from true north to the top edge, while the heading can be
    /// trusted and turned to true north.
    pub heading: Option<u16>,
    /// The member chosen last, by its device, which the face keeps while it has a position.
    pub selected: Option<[u8; 8]>,
    pub now: Micros,
}

/// What a tap on the face asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tap {
    /// The member with this device.
    Select([u8; 8]),
    /// The group's members, from the face with no member placed.
    Members,
    /// The group screen, from the face with no group.
    Group,
}

/// A member with a position the face may show.
#[derive(Clone, Copy, Debug)]
struct Placed {
    id: u8,
    device: [u8; 8],
    coordinates: (i32, i32),
    /// How old the position is, or `None` while this device has no UTC to age it by.
    age: Option<Micros>,
    /// How long ago this device last heard the member itself.
    heard: Option<Micros>,
}

type Placements = Vec<Placed, { IDS as usize }>;

enum State {
    NoGroup,
    NoPositions,
    NoOwnFix,
    /// This device's position, from its current fix.
    Ring((i32, i32)),
}

/// How far east of true north magnetic north lies where the device last had a fix, in the
/// clock's year. `None` without a fix since start-up, without a trusted UTC, or where the
/// magnetic model does not hold.
#[must_use]
pub fn declination(gnss: &Gnss, clock: &ClockState) -> Option<f32> {
    let (latitude, longitude) = gnss.position?;
    let utc = clock.utc.filter(|_| !clock.stopped)?;
    octowhere_motion::declination::declination(
        latitude as f32 * 1e-7,
        longitude as f32 * 1e-7,
        0.0,
        octowhere_motion::declination::decimal_year(utc),
    )
}

/// The compass's heading turned to true north, while the compass gives one it trusts: not
/// calibrating, held too near upright, or disturbed.
#[must_use]
pub fn true_heading(compass: &CompassView, declination: Option<f32>) -> Option<f32> {
    let magnetic = compass
        .heading_decidegrees
        .filter(|_| compass.live && !compass.disturbed)?;
    Some(around(f32::from(magnetic) / 10.0 + declination?))
}

/// `heading` in whole degrees, kept at `shown` until it moves more than three quarters of a
/// degree from it, so that a heading at rest does not flicker between two.
#[must_use]
pub fn hold(shown: Option<u16>, heading: Option<f32>) -> Option<u16> {
    let heading = heading?;
    if let Some(shown) = shown {
        let apart = around(heading - f32::from(shown));
        if apart.min(360.0 - apart) <= 0.75 {
            return Some(shown);
        }
    }
    Some(libm::roundf(heading) as u16 % 360)
}

/// The bearing from `from` to `to`, in degrees clockwise from true north, and the distance
/// between them in metres, on a sphere. Positions are in degrees × 10⁷.
#[must_use]
fn bearing_distance(from: (i32, i32), to: (i32, i32)) -> (f32, f32) {
    const EARTH: f32 = 6_371_008.8;
    let radians = |e7: i64| (e7 as f32 * 1e-7).to_radians();
    let (phi1, phi2) = (radians(from.0.into()), radians(to.0.into()));
    // The differences are taken in integers, where they are exact.
    let delta_phi = radians(i64::from(to.0) - i64::from(from.0));
    let delta_lambda = radians(
        (i64::from(to.1) - i64::from(from.1) + 1_800_000_000).rem_euclid(3_600_000_000)
            - 1_800_000_000,
    );
    let (sin_half_phi, sin_half_lambda) =
        (libm::sinf(delta_phi / 2.0), libm::sinf(delta_lambda / 2.0));
    let (cos1, cos2) = (libm::cosf(phi1), libm::cosf(phi2));
    let a = sin_half_phi * sin_half_phi + cos1 * cos2 * sin_half_lambda * sin_half_lambda;
    let distance = 2.0 * EARTH * libm::asinf(libm::sqrtf(a.clamp(0.0, 1.0)));
    // The bearing's northward part, rewritten so that it does not subtract two near-equal
    // products when the points are close.
    let north =
        libm::sinf(delta_phi) + libm::sinf(phi1) * cos2 * 2.0 * sin_half_lambda * sin_half_lambda;
    let east = libm::sinf(delta_lambda) * cos2;
    let bearing = around(libm::atan2f(east, north).to_degrees());
    (bearing, distance)
}

/// The members other than this device with a position the protocol still holds.
fn placed(group: &GroupView, now: Micros) -> Placements {
    let elapsed = |at: i64| (now as i64 - at).max(0) as Micros;
    group
        .members()
        .filter(|&(id, _)| id != group.own)
        .filter_map(|(id, member)| {
            let age = match member.position {
                Position::Never => return None,
                Position::Unknown => None,
                Position::At(at) => Some(elapsed(at)),
            };
            if age.is_some_and(|age| age > EXPIRES) {
                return None;
            }
            Some(Placed {
                id,
                device: member.device,
                coordinates: member.coordinates?,
                age,
                heard: member.heard.map(elapsed),
            })
        })
        .collect()
}

/// The member whose device `selected` names while it is placed, or else the freshest position.
fn chosen(placed: &[Placed], selected: Option<[u8; 8]>) -> Option<&Placed> {
    placed
        .iter()
        .find(|each| Some(each.device) == selected)
        .or_else(|| freshest(placed.iter()))
}

fn freshest<'a>(placed: impl Iterator<Item = &'a Placed>) -> Option<&'a Placed> {
    placed.min_by_key(|each| (each.age.is_none(), each.age, each.id))
}

fn state(context: &Context, placed: &Placements) -> State {
    match context.group {
        None => State::NoGroup,
        Some(_) if placed.is_empty() => State::NoPositions,
        Some(_) => match context.gnss.position.filter(|_| context.gnss.fix) {
            Some(own) => State::Ring(own),
            None => State::NoOwnFix,
        },
    }
}

/// What a tap at `point` asks of the face as `context` draws it.
#[must_use]
pub fn tap(context: &Context, point: Point) -> Option<Tap> {
    let placed = context
        .group
        .map(|group| placed(group, context.now))
        .unwrap_or_default();
    let next = || {
        let current = chosen(&placed, context.selected)?.id;
        placed
            .iter()
            .find(|each| each.id > current)
            .or(placed.first())
            .map(|each| Tap::Select(each.device))
    };
    match state(context, &placed) {
        State::NoGroup => BUTTON.contains(point).then_some(Tap::Group),
        State::NoPositions => BUTTON.contains(point).then_some(Tap::Members),
        State::NoOwnFix => COORDINATES.contains(point).then(next).flatten(),
        State::Ring(_) => {
            let (dx, dy) = (point.x - CENTER.x, point.y - CENTER.y);
            (dx * dx + dy * dy < MIDDLE_RADIUS * MIDDLE_RADIUS)
                .then(next)
                .flatten()
        }
    }
}

/// Builds the face into `list`, and returns the device of the member it shows selected.
pub fn build(
    context: &Context,
    font: &FontdueRenderer<'static, Color>,
    list: &mut List,
) -> Option<[u8; 8]> {
    list.clear();
    list.set_backdrop(Backdrop::Grid(context.heading.unwrap_or(0)));
    let placed = context
        .group
        .map(|group| placed(group, context.now))
        .unwrap_or_default();
    let selected = chosen(&placed, context.selected).copied();
    for each in &placed {
        due(list, each, context.now);
    }
    let members = context.group.map_or(0, GroupView::count);
    let counts = || {
        format(format_args!(
            "{:02} POSITION{} / {members:02} MEMBER{}",
            placed.len(),
            plural(placed.len()),
            plural(members),
        ))
    };
    match state(context, &placed) {
        State::NoGroup => {
            frame(list, "NO GROUP", context.heading);
            absent(
                list,
                "NO GROUP",
                ["POSITIONS ARE SHARED", "WITHIN A GROUP"],
                "VIEW GROUP",
            );
        }
        State::NoPositions => {
            let counts = format(format_args!(
                "{members:02} MEMBER{} / 00 POSITIONS",
                plural(members)
            ));
            frame(list, &counts, context.heading);
            absent(
                list,
                "NO POSITIONS",
                ["MEMBERS MAY STILL BE HEARD", "POSITIONS NEED A GNSS FIX"],
                "VIEW MEMBERS",
            );
        }
        State::NoOwnFix => {
            frame(list, &counts(), context.heading);
            if let (Some(selected), Some(group)) = (&selected, context.group) {
                coordinates(list, group, selected, font);
            }
        }
        State::Ring(own) => {
            header(list, &counts(), context.heading);
            ring(
                list,
                context,
                &placed,
                selected.map(|each| each.id),
                own,
                font,
            );
            if let (Some(selected), Some(group)) = (&selected, context.group) {
                middle(list, context, group, selected, own, font);
            }
        }
    }
    selected.map(|each| each.device)
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "S" }
}

/// Notes when what the face shows of `placed` next changes: its age's next value, its glyph
/// turning to the hourglass, and its position expiring.
fn due(list: &mut List, placed: &Placed, now: Micros) {
    if let Some(age) = placed.age {
        list.changes_at(now + age_due(age));
        if age < RECENT {
            list.changes_at(now + RECENT - age);
        }
        list.changes_at(now + EXPIRES - age + 1);
    }
    if let Some(heard) = placed.heard {
        list.changes_at(now + age_due(heard));
    }
}

/// The title, the counts and the heading's caption.
fn header(list: &mut List, counts: &str, heading: Option<u16>) {
    list.centred("MEMBERS", 233, 103, Face::Title, 24, chrome::WHITE);
    list.centred(counts, 233, 136, Face::Mono, 12, chrome::GRAY);
    match heading {
        Some(heading) => list.centred(
            &format(format_args!("{heading:03}° TRUE / FORWARD")),
            233,
            156,
            Face::Mono,
            11,
            chrome::GRAY,
        ),
        None => list.centred(
            "NORTH UP / NO HEADING",
            233,
            156,
            Face::Mono,
            11,
            chrome::ORANGE,
        ),
    }
}

/// The ring whole, the north mark and the header, as every state without nodes has them.
fn frame(list: &mut List, counts: &str, heading: Option<u16>) {
    list.arc(ring_arc(WHOLE));
    north(list, heading);
    header(list, counts, heading);
}

fn ring_arc(span: (i16, i16)) -> Arc {
    Arc {
        center: CENTER,
        radius: (RING * 4.0) as u16,
        width: 4,
        span,
        color: chrome::GRAY,
    }
}

/// The rim's N, where true north lies on screen.
fn north(list: &mut List, heading: Option<u16>) {
    let angle = -f32::from(heading.unwrap_or(0));
    list.push(Shape::Turned(Turned {
        text: line("N"),
        face: Face::Mono,
        size: 11,
        color: chrome::LIME,
        center: on_circle(angle, NORTH_RADIUS),
        angle: tenths(angle),
    }));
}

/// What the face says in place of the ring's members, and the control that leads to them.
fn absent(list: &mut List, heading: &str, lines: [&str; 2], control: &str) {
    list.centred(heading, 233, 180, Face::Kh, 30, chrome::GRAY);
    for (line, top) in lines.into_iter().zip([228, 256]) {
        list.centred(line, 233, top, Face::Sans, 16, chrome::WHITE);
    }
    outline(list, BUTTON);
    list.centred(control, 233, 338, Face::Kh, 20, chrome::WHITE);
}

/// The selected member's last coordinates and their age, while this device has no fix to
/// place it from.
fn coordinates(
    list: &mut List,
    group: &GroupView,
    selected: &Placed,
    font: &FontdueRenderer<'static, Color>,
) {
    list.centred("NO OWN FIX", 233, 180, Face::Kh, 30, chrome::ORANGE);
    list.centred(
        "DISTANCE FROM HERE UNAVAILABLE",
        233,
        223,
        Face::Mono,
        13,
        chrome::GRAY,
    );
    outline(list, COORDINATES);
    if let Some(member) = group.member(selected.id) {
        name(
            list,
            member.name.as_str(),
            233,
            265,
            Align::Centre,
            280,
            font,
        );
    }
    let (latitude, longitude) = selected.coordinates;
    list.centred(
        &format(format_args!(
            "{}   {}",
            Degrees(latitude, ['N', 'S']),
            Degrees(longitude, ['E', 'W'])
        )),
        233,
        305,
        Face::Mono,
        16,
        chrome::WHITE,
    );
    let age_text = match selected.age {
        Some(elapsed) => format(format_args!("POSITION {} OLD", age(elapsed))),
        None => line("POSITION AGE UNKNOWN"),
    };
    list.centred(&age_text, 233, 336, Face::Mono, 13, chrome::GRAY);
    list.centred(
        "LAST COORDINATES / NO RELATIVE MARKERS",
        233,
        408,
        Face::Mono,
        11,
        chrome::GRAY,
    );
}

/// Degrees × 10⁷ to four places, with the hemisphere's letter.
struct Degrees(i32, [char; 2]);

impl core::fmt::Display for Degrees {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let tenths_of_thousandths = (self.0.unsigned_abs() + 500) / 1_000;
        let letter = self.1[usize::from(self.0 < 0)];
        write!(
            f,
            "{}.{:04} {letter}",
            tenths_of_thousandths / 10_000,
            tenths_of_thousandths % 10_000
        )
    }
}

/// What sits on the rim: one member with its node and label, or a sector of members whose labels
/// would overlap, drawn as an arc over their bearings with a summary of them. The selected member
/// keeps its own node and label, at its own bearing, in whichever it falls in.
#[derive(Clone, Copy, Debug)]
struct Rim {
    /// Where its members' bearings start on screen, in degrees clockwise from up, and how far
    /// they run clockwise from there: the least arc that holds them all.
    start: f32,
    extent: f32,
    /// Its members' ids, as bits, the selected one's included.
    ids: u32,
    /// The selected member's id and its angle on screen, if it is among them.
    selected: Option<(u8, f32)>,
    /// What its node, labels and arc take of the rim, as angles clockwise from `start`; the
    /// first may be negative.
    covers: (f32, f32),
}

impl Rim {
    fn count(&self) -> u32 {
        self.ids.count_ones()
    }

    fn is_sector(&self) -> bool {
        self.count() > 1
    }

    fn holds(&self, placed: &Placed) -> bool {
        self.ids & 1 << placed.id != 0
    }

    fn middle(&self) -> f32 {
        around(self.start + self.extent / 2.0)
    }
}

/// A member's rim label: its id and its position's age.
fn member_label(placed: &Placed) -> Line {
    let age_text = placed.age.map_or_else(|| line("--"), age);
    format(format_args!("{:02} / {age_text}", placed.id))
}

/// The youngest and oldest of the ages of `members`, which a mean would misstate, and whether
/// any is unknown, which is never taken for young.
fn age_range<'a>(members: impl Iterator<Item = &'a Placed>) -> Line {
    let mut known: Option<(Micros, Micros)> = None;
    let mut unknown = false;
    for each in members {
        match each.age {
            Some(age) => {
                known =
                    Some(known.map_or((age, age), |(least, most)| (least.min(age), most.max(age))));
            }
            None => unknown = true,
        }
    }
    let Some((least, most)) = known else {
        return line("AGE UNKNOWN");
    };
    let (least, most) = (age(least), age(most));
    let mut range = if least == most {
        least
    } else {
        format(format_args!("{least}-{most}"))
    };
    if unknown {
        _ = range.push_str(" / UNKNOWN");
    }
    range
}

/// A sector's summary: how many members it holds and their ages' range.
fn summary(rim: &Rim, placed: &[Placed]) -> Line {
    format(format_args!(
        "{:02} PEERS / {}",
        rim.count(),
        age_range(placed.iter().filter(|each| rim.holds(each)))
    ))
}

/// The degrees either side of its middle that text `width` pixels wide takes when it runs
/// along the circle of `radius`, its inner corners included.
fn half_angle(width: f32, radius: f32) -> f32 {
    libm::atan2f(width / 2.0, radius - LABEL_HALF_HEIGHT).to_degrees()
}

/// `degrees` brought into `-180.0..180.0`.
fn signed(degrees: f32) -> f32 {
    around(degrees + 180.0) - 180.0
}

/// Gathers the members at `angles` on screen into what the rim shows, so that no two labels'
/// ink comes within [`LABEL_GAP`] of each other: the neighbours that overlap most merge first,
/// into a sector, and the selected member, which never moves, takes its neighbours into its own.
/// Only the members' angles relative to each other count, so turning the face does not regroup
/// it.
fn crowd(
    placed: &[Placed],
    angles: &[f32],
    selected: Option<u8>,
    font: &FontdueRenderer<'static, Color>,
) -> Vec<Rim, { IDS as usize }> {
    let label = style(font, chrome::WHITE, 11, Face::Mono.index());
    let width = |text: &str| label.baseline_bounds(text, Point::zero()).size.width as f32;
    let node = libm::atan2f(
        NODE_HALF as f32 * core::f32::consts::SQRT_2,
        NODE_RADIUS - NODE_HALF as f32,
    )
    .to_degrees();
    let member = |id: u8| placed.iter().find(|each| each.id == id);
    let covers = |rim: &Rim| -> (f32, f32) {
        let (mut from, mut to) = (0.0, rim.extent);
        let mut take = |at: f32, half: f32| {
            from = f32::min(from, at - half);
            to = f32::max(to, at + half);
        };
        if let Some((id, angle)) = rim.selected {
            let text = member(id).map(member_label).unwrap_or_default();
            take(
                around(angle - rim.start),
                half_angle(width(&text), LABEL_RADIUS).max(node),
            );
        } else if rim.is_sector() {
            take(
                rim.extent / 2.0,
                half_angle(width(&summary(rim, placed)), SUMMARY_RADIUS),
            );
        } else if let Some(each) = placed.iter().find(|each| rim.holds(each)) {
            take(
                0.0,
                half_angle(width(&member_label(each)), LABEL_RADIUS).max(node),
            );
        }
        (from, to)
    };
    let gap = (LABEL_GAP / (SUMMARY_RADIUS - LABEL_HALF_HEIGHT)).to_degrees();
    let mut rims: Vec<Rim, { IDS as usize }> = placed
        .iter()
        .zip(angles)
        .map(|(each, &angle)| {
            let mut rim = Rim {
                start: angle,
                extent: 0.0,
                ids: 1 << each.id,
                selected: (Some(each.id) == selected).then_some((each.id, angle)),
                covers: (0.0, 0.0),
            };
            rim.covers = covers(&rim);
            rim
        })
        .collect();
    rims.sort_unstable_by(|a, b| a.start.total_cmp(&b.start));
    while rims.len() > 1 {
        let n = rims.len();
        let overlap = |i: usize| {
            let (a, b) = (&rims[i], &rims[(i + 1) % n]);
            gap - signed(b.start + b.covers.0 - (a.start + a.covers.1))
        };
        let Some(i) = (0..n)
            .map(|i| (i, overlap(i)))
            .filter(|&(_, overlap)| overlap > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
        else {
            break;
        };
        let j = (i + 1) % n;
        let (a, b) = (rims[i], rims[j]);
        let mut merged = Rim {
            start: a.start,
            extent: around(b.start - a.start) + b.extent,
            ids: a.ids | b.ids,
            selected: a.selected.or(b.selected),
            covers: (0.0, 0.0),
        };
        merged.covers = covers(&merged);
        rims[i] = merged;
        rims.remove(j);
        rims.sort_unstable_by(|a, b| a.start.total_cmp(&b.start));
    }
    rims
}

/// The ring with its nodes, gapped under each, the sectors, the forward tick, the north mark
/// and the middle's heading marker.
fn ring(
    list: &mut List,
    context: &Context,
    placed: &Placements,
    selected: Option<u8>,
    own: (i32, i32),
    font: &FontdueRenderer<'static, Color>,
) {
    // Every angle on screen is an absolute bearing less the one heading, so that nothing on
    // the rim drifts from another as the face turns.
    let turn = f32::from(context.heading.unwrap_or(0));
    let angles: Vec<f32, { IDS as usize }> = placed
        .iter()
        .map(|each| around(bearing_distance(own, each.coordinates).0 - turn))
        .collect();
    let rims = crowd(placed, &angles, selected, font);
    // Each node's member, its angle on screen, its centre and the ring's gap under it: every
    // member on its own, and the selected member in its sector.
    let nodes: Vec<(u8, f32, Point, (f32, f32)), { IDS as usize }> = rims
        .iter()
        .filter_map(|rim| {
            let (id, angle) = match rim.selected {
                Some(selected) => selected,
                None if !rim.is_sector() => (rim.ids.trailing_zeros() as u8, rim.start),
                None => return None,
            };
            let node = on_circle(angle, NODE_RADIUS);
            Some((id, angle, node, gap(angle, node, RING)))
        })
        .collect();
    match nodes.len() {
        0 => list.arc(ring_arc(WHOLE)),
        n => {
            for (i, &(_, angle, _, (_, after))) in nodes.iter().enumerate() {
                let (_, next, _, (before, _)) = nodes[(i + 1) % n];
                let mut end = next - before;
                let start = angle + after;
                while end <= start {
                    end += 360.0;
                }
                list.arc(ring_arc((tenths(start - 90.0), tenths(end - 90.0))));
            }
        }
    }
    let up_clear = |from: f32, to: f32| {
        let from_up = signed(from);
        from_up > 1.0 || from_up + (to - from) < -1.0
    };
    let tick_clear = nodes
        .iter()
        .all(|&(_, angle, _, (before, after))| up_clear(angle - before, angle + after))
        && rims
            .iter()
            .filter(|rim| rim.is_sector())
            .all(|rim| up_clear(rim.start, rim.start + rim.extent));
    if tick_clear {
        list.path(&[(932, 60), (932, 112)], 4, chrome::GRAY);
    }
    north(list, context.heading);
    for rim in rims.iter().filter(|rim| rim.is_sector()) {
        let color = if rim.selected.is_some() {
            chrome::LIME
        } else {
            chrome::GRAY
        };
        sector(list, rim, &nodes, color);
        if rim.selected.is_none() {
            list.push(Shape::Turned(Turned {
                text: summary(rim, placed),
                face: Face::Mono,
                size: 11,
                color,
                center: on_circle(rim.middle(), SUMMARY_RADIUS),
                angle: tenths(rim.middle()),
            }));
        }
    }
    for &(id, angle, node, _) in &nodes {
        let Some(each) = placed.iter().find(|each| each.id == id) else {
            continue;
        };
        let color = if Some(id) == selected {
            chrome::LIME
        } else {
            chrome::WHITE
        };
        node_frame(list, node, each.age.is_some_and(|age| age < RECENT), color);
        list.push(Shape::Turned(Turned {
            text: member_label(each),
            face: Face::Mono,
            size: 11,
            color,
            center: on_circle(angle, LABEL_RADIUS),
            angle: tenths(angle),
        }));
    }
    if let Some(rim) = rims
        .iter()
        .find(|rim| rim.selected.is_some() && rim.is_sector())
    {
        let others = placed
            .iter()
            .filter(|each| rim.holds(each) && Some(each.id) != selected);
        list.centred(
            &format(format_args!(
                "SECTOR +{:02} / {}",
                rim.count() - 1,
                age_range(others)
            )),
            233,
            SECTOR_TOP,
            Face::Mono,
            11,
            chrome::GRAY,
        );
    }
    list.arc(Arc {
        center: CENTER,
        radius: 106,
        width: 4,
        span: WHOLE,
        color: chrome::GRAY,
    });
    if context.heading.is_some() {
        list.polygon(&ARROW, chrome::LIME);
    } else {
        list.path(&[(900, 932), (964, 932)], 4, chrome::GRAY);
        list.path(&[(932, 900), (932, 964)], 4, chrome::GRAY);
    }
}

/// A sector's arc over its members' bearings, broken where a node sits on it, and the radial
/// marks at its ends; one whose members share a bearing is a single mark.
fn sector(list: &mut List, rim: &Rim, nodes: &[(u8, f32, Point, (f32, f32))], color: Color) {
    let end = rim.start + rim.extent;
    // The angles under the node a sector holds, where neither its arc nor its marks are drawn.
    let under: Option<(f32, f32)> = rim.selected.and_then(|(id, _)| {
        let &(_, angle, node, _) = nodes.iter().find(|held| held.0 == id)?;
        let (before, after) = gap(angle, node, SECTOR_RADIUS);
        let at = rim.start + around(angle - rim.start);
        Some((at - before, at + after))
    });
    let mut arc = |from: f32, to: f32| {
        if to > from {
            list.arc(Arc {
                center: CENTER,
                radius: (SECTOR_RADIUS * 4.0) as u16,
                width: 4,
                span: (tenths(from - 90.0), tenths(to - 90.0)),
                color,
            });
        }
    };
    match under {
        Some((from, to)) => {
            arc(rim.start, from.min(end));
            arc(to.max(rim.start), end);
        }
        None => arc(rim.start, end),
    }
    for at in [rim.start, end] {
        if under.is_some_and(|(from, to)| (from..=to).contains(&at)) {
            continue;
        }
        let quarter = |radius: f32| {
            let (sin, cos) = libm::sincosf(at.to_radians());
            (
                libm::roundf((CX + sin * radius) * 4.0) as i16,
                libm::roundf((CY - cos * radius) * 4.0) as i16,
            )
        };
        list.path(
            &[
                quarter(SECTOR_RADIUS - SECTOR_MARK),
                quarter(SECTOR_RADIUS + SECTOR_MARK),
            ],
            4,
            color,
        );
        if rim.extent == 0.0 {
            break;
        }
    }
}

/// A node's upright frame at `node` with its freshness glyph: a solid square for a recent
/// position, an hourglass for an older one.
fn node_frame(list: &mut List, node: Point, recent: bool, color: Color) {
    let (x, y) = ((node.x * 4) as i16, (node.y * 4) as i16);
    let side = (NODE_HALF * 4 - 2) as i16;
    list.path(
        &[
            (x - side, y - side),
            (x + side, y - side),
            (x + side, y + side),
            (x - side, y + side),
            (x - side, y - side),
        ],
        4,
        color,
    );
    if recent {
        list.fill(
            Rectangle::new(node - Point::new_equal(3), Size::new_equal(6)),
            color,
        );
    } else {
        list.path(
            &[
                (x - 14, y - 18),
                (x + 14, y - 18),
                (x - 14, y + 18),
                (x + 14, y + 18),
                (x - 14, y - 18),
            ],
            4,
            color,
        );
    }
}

/// How far, in degrees either side of a node at `angle`, the circle of `radius` runs under its
/// frame.
fn gap(angle: f32, node: Point, radius: f32) -> (f32, f32) {
    let outside = |degrees: f32| {
        let (sin, cos) = libm::sincosf(degrees.to_radians());
        let (x, y) = (CX + sin * radius, CY - cos * radius);
        (x - node.x as f32).abs().max((y - node.y as f32).abs()) > NODE_HALF as f32 + 0.5
    };
    let side = |direction: f32| {
        let (mut inside, mut out) = (0.0, 12.0);
        for _ in 0..12 {
            let middle = (inside + out) / 2.0;
            if outside(angle + direction * middle) {
                out = middle;
            } else {
                inside = middle;
            }
        }
        out
    };
    (side(-1.0), side(1.0))
}

/// The selected member across the middle: its index, name and true bearing on the left, the
/// distance on the right, and below them its position's age, when this device last heard it,
/// and this device's own fix.
fn middle(
    list: &mut List,
    context: &Context,
    group: &GroupView,
    selected: &Placed,
    own: (i32, i32),
    font: &FontdueRenderer<'static, Color>,
) {
    let (bearing, metres) = bearing_distance(own, selected.coordinates);
    list.left(
        &format(format_args!("[{:02}]", selected.id)),
        94,
        183,
        Face::Mono,
        12,
        chrome::LIME,
    );
    if let Some(member) = group.member(selected.id) {
        name(
            list,
            member.name.as_str(),
            94,
            209,
            Align::Left,
            NAME_WIDTH,
            font,
        );
    }
    list.left(
        &format(format_args!(
            "{:03}° TRUE",
            libm::roundf(bearing) as u16 % 360
        )),
        94,
        248,
        Face::Mono,
        15,
        chrome::WHITE,
    );
    let (figure, unit) = distance(metres);
    let size = fit(&figure, Face::Kh, DISTANCE_SIZES, DISTANCE_WIDTH, font);
    list.left(&figure, 289, 203, Face::Kh, size, chrome::WHITE);
    list.left(unit, 289, 251, Face::Mono, 12, chrome::GRAY);

    list.left("POSITION", 128, 296, Face::Mono, 11, chrome::GRAY);
    let position = match selected.age {
        Some(elapsed) => format(format_args!("{} OLD", age(elapsed))),
        None => line("UNKNOWN"),
    };
    list.left(&position, 128, 318, Face::Kh, 18, chrome::WHITE);
    list.left("DIRECT", 258, 296, Face::Mono, 11, chrome::GRAY);
    match selected.heard {
        Some(heard) => list.left(
            &format(format_args!("{} AGO", age(heard))),
            258,
            318,
            Face::Kh,
            18,
            chrome::VIOLET,
        ),
        None => list.left("NEVER", 258, 318, Face::Kh, 18, chrome::GRAY),
    }

    let gnss = context.gnss;
    let fix_age = gnss
        .health
        .last_fix
        .map(|at| context.now.saturating_sub(at));
    if let Some(fix_age) = fix_age {
        list.changes_at(context.now + age_due(fix_age));
    }
    let mut own_fix = Line::new();
    _ = core::fmt::Write::write_str(&mut own_fix, "OWN FIX");
    if let Some(fix_age) = fix_age {
        _ = core::fmt::Write::write_fmt(&mut own_fix, format_args!(" {}", age(fix_age)));
    }
    match gnss.hdop_milli {
        Some(milli) => {
            let tenths = (milli + 50) / 100;
            _ = core::fmt::Write::write_fmt(
                &mut own_fix,
                format_args!(" / HDOP {}.{}", tenths / 10, tenths % 10),
            );
        }
        None => _ = core::fmt::Write::write_str(&mut own_fix, " / HDOP --"),
    }
    list.centred(&own_fix, 233, 348, Face::Mono, 14, chrome::GRAY);
}

/// A distance as a figure and its unit: whole metres below a kilometre, kilometres to a tenth
/// below a hundred, and whole kilometres past that.
fn distance(metres: f32) -> (Line, &'static str) {
    let whole = libm::roundf(metres);
    if whole < 1_000.0 {
        (format(format_args!("{whole:.0}")), "METRES")
    } else if metres < 99_950.0 {
        (
            format(format_args!("{:.1}", metres / 1_000.0)),
            "KILOMETRES",
        )
    } else {
        (
            format(format_args!("{:.0}", metres / 1_000.0)),
            "KILOMETRES",
        )
    }
}

/// The largest size within `sizes` at which `text`'s ink is at most `width` wide, or the
/// smallest.
fn fit(
    text: &str,
    face: Face,
    (largest, smallest): (u8, u8),
    width: u32,
    font: &FontdueRenderer<'static, Color>,
) -> u8 {
    (smallest..=largest)
        .rev()
        .find(|&size| {
            style(font, chrome::WHITE, u32::from(size), face.index())
                .baseline_bounds(text, Point::zero())
                .size
                .width
                <= width
        })
        .unwrap_or(smallest)
}

/// A member's name, as large as fits `width` up to the design's 25 px, sitting where a capital
/// of 25 px would whatever size it takes, and cut short if it fits at none.
fn name(
    list: &mut List,
    text: &str,
    x: i32,
    top: i32,
    align: Align,
    width: u32,
    font: &FontdueRenderer<'static, Color>,
) {
    let size = fit(text, Face::Mono, NAME_SIZES, width, font);
    let at = |size: u8| style(font, chrome::WHITE, u32::from(size), Face::Mono.index());
    let baseline = top + super::text::cap(&at(NAME_SIZES.0));
    let style = at(size);
    let mut shown = line(text);
    while shown.len() > 1 && style.baseline_bounds(&shown, Point::zero()).size.width > width {
        shown.pop();
    }
    list.text(
        Text::new(&shown, Face::Mono, size, chrome::WHITE)
            .at(x, baseline - super::text::cap(&style))
            .align(align)
            .vertical(Vertical::Cap),
    );
}

/// Draws the grid turned `turn` degrees anticlockwise about the panel's centre.
pub fn draw_grid<D: CoverageTarget<Color = Color>>(
    turn: u16,
    target: &mut D,
) -> Result<(), D::Error> {
    let (sin, cos) = libm::sincosf(-f32::from(turn).to_radians());
    let place = |x: f32, y: f32| (CX + cos * x - sin * y, CY + sin * x + cos * y);
    let quarter = |(x, y): (f32, f32)| (libm::roundf(x * 4.0) as i16, libm::roundf(y * 4.0) as i16);
    let lines = (GRID_REACH / PITCH as f32) as i32;
    for k in -lines..=lines {
        let along = (k * PITCH) as f32;
        let half = libm::sqrtf(GRID_REACH * GRID_REACH - along * along);
        for (a, b) in [
            (place(along, -half), place(along, half)),
            (place(-half, along), place(half, along)),
        ] {
            stroke::draw_path(
                &[quarter(a), quarter(b)],
                GRID_LINE_WIDTH,
                chrome::GRID_LINE,
                target,
            );
        }
    }
    for i in -lines..=lines {
        for j in -lines..=lines {
            let (x, y) = ((i * PITCH) as f32, (j * PITCH) as f32);
            if x * x + y * y > GRID_REACH * GRID_REACH {
                continue;
            }
            let (px, py) = place(x, y);
            let mark = Rectangle::new(
                Point::new(libm::roundf(px) as i32 - 1, libm::roundf(py) as i32 - 1),
                Size::new_equal(GRID_MARK),
            );
            target.fill_solid(&mark, chrome::GRID_MARK)?;
        }
    }
    Ok(())
}

/// `degrees` brought into `0.0..360.0`.
fn around(degrees: f32) -> f32 {
    let turn = libm::fmodf(degrees, 360.0);
    if turn < 0.0 { turn + 360.0 } else { turn }
}

/// Tenths of a degree, as the list's angles take them.
fn tenths(degrees: f32) -> i16 {
    libm::roundf(degrees * 10.0) as i16
}

/// The point `radius` from the centre at `degrees` clockwise from up.
fn on_circle(degrees: f32, radius: f32) -> Point {
    let (sin, cos) = libm::sincosf(degrees.to_radians());
    Point::new(
        libm::roundf(CX + sin * radius) as i32,
        libm::roundf(CY - cos * radius) as i32,
    )
}

/// A one-pixel outline round `area` that leaves the grid inside showing: its four sides as
/// fills, where a path would measure every pixel inside.
fn outline(list: &mut List, area: Rectangle) {
    let (x0, y0) = (area.top_left.x, area.top_left.y);
    let (x1, y1) = (x0 + area.size.width as i32, y0 + area.size.height as i32);
    for side in [
        rect(x0, y0, x1, y0 + 1),
        rect(x0, y1 - 1, x1, y1),
        rect(x0, y0 + 1, x0 + 1, y1 - 1),
        rect(x1 - 1, y0 + 1, x1, y1 - 1),
    ] {
        list.fill(side, chrome::GRAY);
    }
}

fn line(text: &str) -> Line {
    format(format_args!("{text}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUBLIN: (i32, i32) = (533_498_000, -62_603_000);

    /// The point `metres` from `from` at `bearing`, in doubles, as the reference.
    fn destination(from: (i32, i32), bearing: f64, metres: f64) -> (i32, i32) {
        let (phi1, lambda1) = (
            (f64::from(from.0) * 1e-7).to_radians(),
            (f64::from(from.1) * 1e-7).to_radians(),
        );
        let (d, theta) = (metres / 6_371_008.8, bearing.to_radians());
        let phi2 = (phi1.sin() * d.cos() + phi1.cos() * d.sin() * theta.cos()).asin();
        let lambda2 =
            lambda1 + (theta.sin() * d.sin() * phi1.cos()).atan2(d.cos() - phi1.sin() * phi2.sin());
        (
            (phi2.to_degrees() * 1e7).round() as i32,
            (lambda2.to_degrees() * 1e7).round() as i32,
        )
    }

    #[test]
    fn a_bearing_and_distance_match_the_great_circle() {
        for (bearing, metres) in [
            (25.0, 420.0),
            (217.0, 12.0),
            (285.0, 2_350.0),
            (120.0, 464_000.0),
        ] {
            let (found, distance) = bearing_distance(DUBLIN, destination(DUBLIN, bearing, metres));
            assert!(
                (f64::from(found) - bearing).abs() < 0.1,
                "{bearing}: {found}"
            );
            assert!(
                (f64::from(distance) - metres).abs() < metres * 1e-3 + 0.5,
                "{metres}: {distance}"
            );
        }
        // A quarter of a meridian.
        let (bearing, metres) = bearing_distance((0, 0), (900_000_000, 0));
        assert!(bearing.abs() < 0.01, "{bearing}");
        assert!((metres - 10_007_557.0).abs() < 50.0, "{metres}");
        // Across the antimeridian, east is still east.
        let (bearing, _) = bearing_distance((0, 1_799_000_000), (0, -1_799_000_000));
        assert!((bearing - 90.0).abs() < 0.1, "{bearing}");
    }

    #[test]
    fn a_heading_holds_until_it_moves_most_of_a_degree() {
        assert_eq!(hold(None, Some(61.6)), Some(62));
        assert_eq!(hold(Some(62), Some(62.7)), Some(62));
        assert_eq!(hold(Some(62), Some(61.3)), Some(62));
        assert_eq!(hold(Some(62), Some(62.8)), Some(63));
        assert_eq!(hold(Some(0), Some(359.4)), Some(0));
        assert_eq!(hold(Some(0), Some(359.1)), Some(359));
        assert_eq!(hold(Some(62), None), None);
    }

    #[test]
    fn a_distance_counts_in_the_unit_it_fits() {
        assert_eq!(distance(420.4).0, "420");
        assert_eq!(distance(999.6), (line("1.0"), "KILOMETRES"));
        assert_eq!(distance(12_340.0).0, "12.3");
        assert_eq!(distance(464_000.0), (line("464"), "KILOMETRES"));
    }

    fn font() -> FontdueRenderer<'static, Color> {
        FontdueRenderer::new(
            chrome::FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        )
    }

    /// Members 1 on, at `angles` on screen, each `age` seconds old if it has one.
    fn members(angles: &[(f32, Option<u64>)]) -> (Placements, Vec<f32, { IDS as usize }>) {
        let placed = (1..)
            .zip(angles)
            .map(|(id, &(_, age))| Placed {
                id,
                device: [id; 8],
                coordinates: (0, 0),
                age: age.map(|age| age * SECOND),
                heard: None,
            })
            .collect();
        (placed, angles.iter().map(|&(angle, _)| angle).collect())
    }

    /// Where each rim's ink starts and ends on screen, in order round the face.
    fn spans(rims: &[Rim]) -> std::vec::Vec<(f32, f32)> {
        rims.iter()
            .map(|rim| (rim.start + rim.covers.0, rim.start + rim.covers.1))
            .collect()
    }

    fn assert_apart(rims: &[Rim]) {
        let gap = (LABEL_GAP / (SUMMARY_RADIUS - LABEL_HALF_HEIGHT)).to_degrees();
        let spans = spans(rims);
        for i in 0..spans.len() {
            let next = spans[(i + 1) % spans.len()];
            if spans.len() > 1 {
                let apart = signed(next.0 - spans[i].1);
                assert!(apart >= gap - 1e-3, "{i}: {apart} in {spans:?}");
            }
        }
    }

    #[test]
    fn a_crowded_ring_keeps_every_label_apart_and_the_selection_exact() {
        // 31 members, a dense run of eleven and the rest spread by the golden angle.
        let angles: std::vec::Vec<(f32, Option<u64>)> = (1..32u16)
            .map(|id| {
                let angle = if id < 12 {
                    40.0 + f32::from(id)
                } else {
                    f32::from(id) * 137.5 % 360.0
                };
                (angle, Some(u64::from(id) * 7))
            })
            .collect();
        let (placed, angles) = members(&angles);
        let font = font();
        for selected in [3, 20] {
            for turn in [0.0, 137.0, 359.5] {
                let turned: Vec<f32, { IDS as usize }> =
                    angles.iter().map(|angle| around(angle - turn)).collect();
                let rims = crowd(&placed, &turned, Some(selected), &font);
                assert_apart(&rims);
                let held: u32 = rims.iter().map(|rim| rim.ids).fold(0, |all, ids| {
                    assert_eq!(all & ids, 0, "a member in two places");
                    all | ids
                });
                assert_eq!(held.count_ones(), 31);
                let rim = rims
                    .iter()
                    .find(|rim| rim.selected.is_some())
                    .expect("the selected member is on the rim");
                assert_eq!(
                    rim.selected,
                    Some((selected, turned[usize::from(selected) - 1]))
                );
                assert!(rims.iter().filter(|rim| rim.is_sector()).count() >= 1);
            }
        }
    }

    #[test]
    fn turning_the_face_does_not_regroup_it() {
        let angles: std::vec::Vec<(f32, Option<u64>)> = (0..20u16)
            .map(|i| (f32::from(i * i) * 7.3 % 360.0, Some(30)))
            .collect();
        let (placed, angles) = members(&angles);
        let font = font();
        let groups = |turn: f32| {
            let turned: Vec<f32, { IDS as usize }> =
                angles.iter().map(|angle| around(angle - turn)).collect();
            let mut ids: std::vec::Vec<u32> = crowd(&placed, &turned, Some(4), &font)
                .iter()
                .map(|rim| rim.ids)
                .collect();
            ids.sort_unstable();
            ids
        };
        let first = groups(0.0);
        for turn in [1.0, 45.5, 180.0, 271.25, 359.0] {
            assert_eq!(groups(turn), first, "{turn}");
        }
    }

    #[test]
    fn a_sector_across_north_spans_the_least_arc() {
        let (placed, angles) = members(&[
            (355.0, Some(11)),
            (358.0, Some(20)),
            (2.0, Some(60)),
            (5.0, Some(1_080)),
            (180.0, Some(30)),
        ]);
        let rims = crowd(&placed, &angles, Some(5), &font());
        let sector = rims.iter().find(|rim| rim.is_sector()).unwrap();
        assert_eq!(sector.count(), 4);
        assert!((sector.start - 355.0).abs() < 1e-3, "{sector:?}");
        assert!((sector.extent - 10.0).abs() < 1e-3, "{sector:?}");
        assert_eq!(summary(sector, &placed), "04 PEERS / 11S-18M");
    }

    #[test]
    fn coincident_bearings_are_a_sector_of_no_width() {
        let (placed, angles) = members(&[(90.0, Some(11)); 5]);
        let rims = crowd(&placed, &angles, None, &font());
        assert_eq!(rims.len(), 1);
        assert_eq!((rims[0].count(), rims[0].extent), (5, 0.0));
        // With one of them selected, it keeps its node, and the rest are its sector.
        let rims = crowd(&placed, &angles, Some(2), &font());
        assert_eq!(rims.len(), 1);
        assert_eq!(rims[0].selected, Some((2, 90.0)));
    }

    #[test]
    fn members_with_room_keep_their_own_nodes() {
        let (placed, angles) = members(&[(0.0, Some(1)), (90.0, None), (180.0, Some(3_600))]);
        let rims = crowd(&placed, &angles, Some(1), &font());
        assert_eq!(rims.len(), 3);
        assert!(rims.iter().all(|rim| !rim.is_sector()));
    }

    #[test]
    fn an_age_range_is_its_youngest_and_oldest() {
        let (placed, _) = members(&[(0.0, Some(23)), (0.0, Some(420)), (0.0, Some(60))]);
        assert_eq!(age_range(placed.iter()), "23S-07M");
        assert_eq!(age_range(placed[..1].iter()), "23S");
        let (unknown, _) = members(&[(0.0, Some(23)), (0.0, None)]);
        assert_eq!(age_range(unknown.iter()), "23S / UNKNOWN");
        assert_eq!(age_range(unknown[1..].iter()), "AGE UNKNOWN");
    }

    #[test]
    fn coordinates_read_to_four_places_with_their_hemisphere() {
        let text = format(format_args!(
            "{}   {}",
            Degrees(DUBLIN.0, ['N', 'S']),
            Degrees(DUBLIN.1, ['E', 'W'])
        ));
        assert_eq!(text, "53.3498 N   6.2603 W");
    }
}
