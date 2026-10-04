//! One image of every screen and state the example draws, laid out like the design's screen family
//! board: a section per family, each tile at the panel's own size behind round glass, its name and
//! PNG under it. The layout depends only on [`SECTIONS`], so two revisions' atlases line up tile for
//! tile, and the header names the revision that drew it.

use std::{collections::HashMap, path::Path, process::Command};

use embedded_graphics::{
    Pixel,
    pixelcolor::{Rgb888, RgbColor},
    prelude::{Dimensions, DrawTarget, OriginDimensions, Point, Size},
};
use octowhere_ui::{
    board::{LCD_HEIGHT, LCD_WIDTH},
    chrome::{self, Color, CoverageTarget, FB, FontdueRenderer, FontdueRendererCtx},
    ui::text,
};

/// Each section's heading, and its tiles: the frame's name, then the label set under it.
const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "STARTUP",
        &[
            ("startup-selftest", "SELF TEST / WAITING"),
            ("startup-selftest-building", "SELF TEST / ANSWERING"),
            ("startup-selftest-passed", "SELF TEST / PASSED"),
            ("startup-selftest-failing", "SELF TEST / FAILING"),
            ("startup-selftest-failed", "SELF TEST / FAILED"),
            ("startup-frame-12", "IDENTITY / OPENING"),
            ("startup-frame-38", "IDENTITY / OUTLINE"),
            ("startup-frame-97", "IDENTITY / SETTLED"),
            ("startup-frame-123", "LOGO CARD"),
            ("startup-frame-137", "LOGO CARD / DARK ON LIME"),
            ("startup-fault-003", "FAULT / ONE PART"),
            ("startup-fault-two", "FAULT / TWO PARTS"),
            ("startup-fault-four", "FAULT / FOUR PARTS"),
            ("startup-fault-128", "FAULT / EXIT"),
            ("startup-selftest-scrolling", "SELF TEST / SCROLLING"),
            (
                "startup-selftest-radio-pending",
                "SELF TEST / RADIO PENDING",
            ),
            ("startup-selftest-radio-failed", "SELF TEST / RADIO FAILED"),
            ("startup-selftest-power-hidden", "SELF TEST / POWER HIDDEN"),
            ("startup-selftest-power-returned", "SELF TEST / POWER BACK"),
            ("startup-fault-radio", "FAULT / RADIO"),
            ("startup-fault-power-radio", "FAULT / POWER + RADIO"),
        ],
    ),
    (
        "CLOCK",
        &[
            ("clock", "GNSS / AUTO"),
            ("clock-rtc", "RTC"),
            ("clock-manual", "MANUAL"),
            ("clock-stopped", "STOPPED"),
            ("clock-no-zone", "NO ZONE"),
            ("clock-no-data", "NO DATA"),
            ("clock-longest-zone", "LONGEST ZONE"),
            ("clock-charging", "CHARGING"),
            ("clock-battery-low", "BATTERY LOW"),
            ("clock-battery-unknown", "NO BATTERY READING"),
            ("clock-entering", "ENTERING"),
            ("clock-marking", "ENTERING / MARK"),
            ("clock-swiping", "SWIPING"),
        ],
    ),
    (
        "COMPASS",
        &[
            ("compass-heading", "HEADING"),
            ("compass-heading-359", "HEADING / 359"),
            ("compass-calibrating", "CALIBRATING"),
            ("compass-hold-level", "HOLD LEVEL"),
            ("compass-interference", "INTERFERENCE"),
            ("compass-interference-350", "INTERFERENCE / 350"),
            ("compass-no-data", "NO DATA"),
            ("compass-entering", "ENTERING"),
            ("compass-sweeping", "ENTERING / SWEEP"),
            ("compass-swiping", "SWIPING"),
        ],
    ),
    (
        "SETTINGS",
        &[
            ("panel-pulling", "PULLING DOWN"),
            ("panel-rest", "OVERVIEW / 1"),
            ("panel-scrolling", "OVERVIEW / SCROLLING"),
            ("panel-end", "OVERVIEW / 2"),
            ("picker-offset", "ZONE / OFFSET"),
            ("picker-zone", "ZONE / PICKER"),
            ("picker-zone-scrolled", "ZONE / SCROLLED"),
            ("settings-brightness", "BRIGHTNESS"),
            ("settings-timeout", "TIMEOUT"),
            ("settings-always-on", "ALWAYS ON"),
            ("panel-device", "DEVICE"),
            ("panel-device-end", "DEVICE / END"),
            ("settings-clear", "CLEAR"),
            ("replay-chooser", "REPLAY"),
            ("replay-chooser-magnet", "REPLAY / MAGNET"),
            ("replay-demo-selftest", "REPLAY / SELF TEST"),
            ("replay-demo-fault", "REPLAY / FAULT"),
            ("replay-chooser-radio", "REPLAY / RADIO"),
            ("replay-demo-radio", "REPLAY / RADIO DEMO"),
        ],
    ),
    (
        "AMBIENT + ACTIONS",
        &[
            ("always-on-local", "AOD / LOCAL"),
            ("always-on-stopped", "AOD / STOPPED"),
            ("always-on-no-zone", "AOD / NO ZONE"),
            ("always-on-no-data", "AOD / NO DATA"),
            ("power-off", "POWER OFF"),
            ("power-off-sliding", "POWER OFF / SLIDING"),
            ("power-off-confirmed", "POWER OFF / CONFIRMED"),
        ],
    ),
    (
        "GROUP + NAME",
        &[
            ("panel-page2", "PANEL / 2"),
            ("group-none", "NO GROUP"),
            ("group-home", "GROUP"),
            ("group-members", "MEMBERS"),
            ("member-detail", "MEMBER"),
            ("member-own", "THIS DEVICE"),
            ("remove-unavailable", "REMOVE / UNAVAILABLE"),
            ("leave-drag", "LEAVE / DRAG"),
            ("leave-done", "LEFT"),
            ("leave-save-failed", "LEAVE / SAVE FAILED"),
            ("keyboard-lowercase", "KEYBOARD"),
            ("keyboard-numbers", "KEYBOARD / 123"),
            ("keyboard-pressed", "KEYBOARD / PRESSED"),
            ("name-save-failed", "NAME / SAVE FAILED"),
        ],
    ),
    (
        "PAIRING",
        &[
            ("add-entry", "ADD"),
            ("join-entry", "JOIN"),
            ("join-leave-first", "JOIN / LEAVE FIRST"),
            ("add-search", "SEARCHING"),
            ("join-announce", "ANNOUNCING"),
            ("add-found", "FOUND"),
            ("add-preparing", "PREPARING"),
            ("add-code", "CODE"),
            ("add-confirm-drag", "CODE / DRAG"),
            ("add-stop-code", "STOP"),
            ("join-waiting", "WAITING"),
            ("add-transfer", "SENDING"),
            ("join-saving", "STORING"),
            ("join-finishing", "FINAL REPLY"),
            ("add-done", "ADDED"),
            ("add-create-done", "GROUP CREATED"),
            ("join-confirmation-lost", "STORED / UNCONFIRMED"),
            ("add-mismatch", "CODES DIFFER"),
            ("add-cancelled", "CANCELLED"),
            ("add-full", "GROUP FULL"),
            ("add-no-radio", "NO RADIO"),
        ],
    ),
    (
        "REFRESH + RECOVERY",
        &[
            ("refresh-entry", "REFRESH"),
            ("refresh-running", "REFRESH / LISTENING"),
            ("members-refresh-active", "MEMBERS / LISTENING"),
            ("refresh-found", "REFRESH / LEARNED"),
            ("refresh-known-only", "REFRESH / HEARD"),
            ("refresh-none", "REFRESH / NOTHING NEW"),
            ("refresh-aged", "REFRESH / AGED"),
            ("refresh-stopped", "REFRESH / STOPPED"),
            ("members-refresh-no-radio", "MEMBERS / NO RADIO"),
            ("refresh-no-radio", "REFRESH / NO RADIO"),
            ("refresh-pairing-busy", "REFRESH / PAIRING"),
            ("refresh-no-group", "REFRESH / NO GROUP"),
            ("add-confirmation-lost", "IN A GROUP / UNCONFIRMED"),
            ("founder-check-member", "FOUNDER / CHECK MEMBER"),
            ("founder-pending-hub", "FOUNDER / PENDING"),
            ("founder-group-stored", "FOUNDER / STORED"),
            ("founder-stored-hub", "FOUNDER / GROUP"),
            ("founder-wait-expired", "FOUNDER / EXPIRED"),
            ("founder-replace-warning", "FOUNDER / END WAIT"),
            ("founder-save-failed", "FOUNDER / SAVE FAILED"),
            ("founder-not-stored", "FOUNDER / NOT STORED"),
        ],
    ),
    (
        "EVENTS",
        &[
            ("events-drawer-new", "EVENTS / NEW"),
            ("events-drawer-history", "EVENTS / SCROLLED"),
            ("events-detail-fault", "GNSS / FAULT"),
            ("events-drawer-manage", "MANAGE HISTORY"),
            ("events-drawer-read", "EVENTS / READ"),
            ("events-drawer-active-after-clear", "EVENTS / CLEARED"),
            ("events-drawer-responding", "EVENTS / RESPONDING"),
            ("events-detail-responding", "GNSS / RESPONDING"),
            ("events-drawer-recovering", "EVENTS / RECOVERING"),
            ("events-drawer-refresh-ongoing", "EVENTS / REFRESHING"),
            ("events-detail-refresh-ongoing", "REFRESH / ONGOING"),
            ("events-detail-refresh", "REFRESH / ENDED"),
            ("events-drawer-empty", "EVENTS / EMPTY"),
            ("messages-empty", "MESSAGES / EMPTY"),
            ("events-drawer-no-devices", "EVENTS / NO DEVICES"),
            ("events-toast-quiet-refresh-clock", "TOAST / NO DEVICES"),
            ("events-toast-fault-clock", "TOAST / FAULT"),
            ("events-clock-unread", "CLOCK / UNREAD"),
            ("events-toast-recovering-clock", "TOAST / RECOVERING"),
            ("events-toast-refresh-clock", "TOAST / REFRESH"),
            ("events-toast-woke", "TOAST / WOKE"),
            ("events-aod-unread", "ALWAYS ON / UNREAD"),
        ],
    ),
    (
        "MEMBERS",
        &[
            ("members-bearing-circle-four", "MEMBERS / FOUR"),
            ("members-bearing-circle-one", "MEMBERS / ONE"),
            ("members-heading-unavailable", "MEMBERS / NORTH UP"),
            ("members-selected-next", "MEMBERS / NEXT"),
            ("members-crowded", "MEMBERS / CROWDED"),
            ("members-no-own-fix", "MEMBERS / NO OWN FIX"),
            ("members-none", "MEMBERS / NO POSITIONS"),
            ("members-no-group", "MEMBERS / NO GROUP"),
            ("members-swiping", "MEMBERS / SWIPING"),
        ],
    ),
    (
        "MESSAGES",
        &[
            ("messages-01-inbox", "MESSAGES / INBOX"),
            ("messages-02-group", "MESSAGES / GROUP"),
            ("messages-03-private", "MESSAGES / PRIVATE"),
            ("messages-04-write", "MESSAGES / WRITE"),
            ("messages-05-review", "MESSAGES / REVIEW"),
            ("messages-sent-queued", "MESSAGES / QUEUED"),
            ("messages-sent-delivered", "MESSAGES / DELIVERED"),
            ("messages-06-arrival", "MESSAGES / ARRIVAL"),
            ("messages-07-recipient", "MESSAGES / SEND TO"),
            ("messages-empty", "MESSAGES / EMPTY"),
            ("messages-write-empty", "MESSAGES / NEW DRAFT"),
            ("messages-write-long", "MESSAGES / LONG DRAFT"),
            ("messages-review-long", "MESSAGES / LONG REVIEW"),
        ],
    ),
    (
        "REMOVAL AND REKEY",
        &[
            ("removal-member-detail", "MEMBER / REMOVE"),
            ("removal-01-confirm-remove", "REMOVE / CONFIRM"),
            ("removal-slide-released", "REMOVE / SLIDE SHORT"),
            ("removal-02-removal-ongoing", "REMOVING / OWN"),
            ("removal-own-switched", "REMOVING / SWITCHED"),
            ("removal-03-incoming-request", "REQUEST / PENDING"),
            ("removal-details", "REQUEST / DETAILS"),
            ("removal-09-confirm-decline-pending", "DECLINE / BEFORE"),
            ("removal-declined", "REQUEST / DECLINED"),
            ("removal-toast-request", "TOAST / REQUEST"),
            ("removal-04-after-switch", "REQUEST / SWITCHED"),
            ("removal-05-confirm-return", "DECLINE / AFTER"),
            ("removal-10-decline-expired", "REQUEST / EXPIRED"),
            ("removal-06-rival-requests", "TWO REQUESTS"),
            ("removal-drawer-events", "EVENTS / REMOVALS"),
            ("removal-toast-removed", "TOAST / REMOVED"),
            ("removal-07-this-device-removed", "REMOVED"),
            ("removal-08-pending-member", "MEMBER / PENDING"),
            ("removal-unavailable-underway", "REMOVE / UNDER WAY"),
            ("removal-unavailable-unsaved", "REMOVE / NOT STORED"),
        ],
    ),
];

const COLUMNS: usize = 7;
const TILE: usize = LCD_WIDTH as usize;
const GAP: usize = 24;
const MARGIN: usize = 54;
const WIDTH: usize = 2 * MARGIN + COLUMNS * TILE + (COLUMNS - 1) * GAP;
/// The heights of the page's header, a section's heading, a tile's labels and the footer.
const HEADER: usize = 120;
const HEADING: usize = 64;
const LABELS: usize = 66;
const FOOTER: usize = 64;

/// The page, the tiles behind the glass, and the section rules.
const PAGE: Rgb888 = Rgb888::new(0x12, 0x13, 0x16);
const TILE_BACK: Rgb888 = Rgb888::new(0x1c, 0x1c, 0x1c);
const RULE: Rgb888 = Rgb888::new(0x2c, 0x2e, 0x33);

/// The atlas of `frames`, drawn by `revision`. Every frame `SECTIONS` names must be there.
pub fn write(frames: &HashMap<String, Box<FB>>, revision: &str, path: &Path) {
    let rows = |tiles: usize| tiles.div_ceil(COLUMNS);
    let height = HEADER
        + SECTIONS
            .iter()
            .map(|(_, tiles)| HEADING + rows(tiles.len()) * (TILE + LABELS))
            .sum::<usize>()
        + FOOTER;
    let mut page = Page {
        pixels: vec![PAGE; WIDTH * height],
        height,
    };
    let glass = glass();
    let white = Rgb888::from(chrome::WHITE);
    let gray = Rgb888::from(chrome::GRAY);
    let lime = Rgb888::from(chrome::LIME);

    page.text(MARGIN, 64, "OCTOWHERE / SCREEN SYSTEM", 36, white);
    let about = format!("{revision}  /  {TILE} PX NATIVE TILES  /  FIRMWARE DRAWING CODE");
    page.text_right(WIDTH - MARGIN, 60, &about, 14, gray);

    let mut top = HEADER;
    for (number, (heading, tiles)) in SECTIONS.iter().enumerate() {
        let heading = format!("{:02} / {heading}", number + 1);
        page.text(MARGIN, top + 34, &heading, 22, lime);
        page.fill(MARGIN + 340, top + 26, WIDTH - 2 * MARGIN - 340, 2, RULE);
        top += HEADING;
        for (index, &(name, label)) in tiles.iter().enumerate() {
            let fb = frames
                .get(name)
                .unwrap_or_else(|| panic!("the atlas names `{name}`, which nothing draws"));
            let x = MARGIN + (index % COLUMNS) * (TILE + GAP);
            let y = top + index / COLUMNS * (TILE + LABELS);
            page.tile(x, y, fb, &glass);
            page.text(x, y + TILE + 28, label, 18, white);
            page.text(x, y + TILE + 50, name, 13, gray);
        }
        top += rows(tiles.len()) * (TILE + LABELS);
    }
    page.text(
        MARGIN,
        top + 36,
        "THE READINGS ARE SYNTHETIC FIXTURES  /  UNDER EACH NAME, ITS PNG FROM THE RENDER EXAMPLE",
        13,
        gray,
    );
    page.save(path);
}

/// The revision the working tree is at, with its commit date and whether it has changes, or
/// `UNKNOWN REVISION` outside git.
pub fn revision() -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(env!("CARGO_MANIFEST_DIR"))
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    match (
        git(&["describe", "--always", "--dirty=+CHANGES", "--abbrev=7"]),
        git(&["log", "-1", "--format=%cs"]),
    ) {
        (Some(described), Some(date)) => format!("{}  /  {date}", described.to_uppercase()),
        _ => "UNKNOWN REVISION".into(),
    }
}

/// How much of each pixel the round glass shows, out of 255, antialiased at its edge.
fn glass() -> Vec<u8> {
    let radius = TILE as f32 / 2.0;
    (0..TILE * TILE)
        .map(|index| {
            let x = (index % TILE) as f32 + 0.5 - radius;
            let y = (index / TILE) as f32 + 0.5 - radius;
            let inside = (radius - x.hypot(y) + 0.5).clamp(0.0, 1.0);
            (inside * 255.0).round() as u8
        })
        .collect()
}

struct Page {
    pixels: Vec<Rgb888>,
    height: usize,
}

impl Page {
    fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, color: Rgb888) {
        for row in y..y + height {
            self.pixels[row * WIDTH + x..row * WIDTH + x + width].fill(color);
        }
    }

    fn mix(&mut self, x: usize, y: usize, coverage: u8, color: Rgb888) {
        let pixel = &mut self.pixels[y * WIDTH + x];
        *pixel = mix(*pixel, color, coverage);
    }

    fn tile(&mut self, x: usize, y: usize, fb: &FB, glass: &[u8]) {
        self.fill(x, y, TILE, TILE, TILE_BACK);
        for (index, &shown) in glass.iter().enumerate() {
            let (column, row) = (index % TILE, index / TILE);
            let point = Point::new(column as i32, row as i32);
            let color = Rgb888::from(fb.pixel(point).expect("inside the panel"));
            self.mix(x + column, y + row, shown, color);
        }
    }

    /// Sets `text` with its pen starting at `x` on `baseline`, in PP Fraktion Mono.
    fn text(&mut self, x: usize, baseline: usize, text: &str, size: u32, color: Rgb888) {
        let style = style(size);
        let mut ink = Ink::new(
            style.advance(text).ceil() as usize + 2 * size as usize,
            size,
        );
        let pen = Point::new(size as i32, ink.baseline);
        style
            .draw_on_baseline(text, pen, &mut ink)
            .expect("setting a label");
        let left = x as i32 - size as i32;
        let top = baseline as i32 - ink.baseline;
        for (index, &coverage) in ink.coverage.iter().enumerate() {
            let (column, row) = (
                left + (index % ink.width) as i32,
                top + (index / ink.width) as i32,
            );
            let inside =
                (0..WIDTH as i32).contains(&column) && (0..self.height as i32).contains(&row);
            if coverage > 0 && inside {
                self.mix(column as usize, row as usize, coverage, color);
            }
        }
    }

    /// Sets `text` so that its pen ends at `right`.
    fn text_right(&mut self, right: usize, baseline: usize, text: &str, size: u32, color: Rgb888) {
        let width = style(size).advance(text).round() as usize;
        self.text(right - width, baseline, text, size, color);
    }

    fn save(&self, path: &Path) {
        let bytes: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|pixel| [pixel.r(), pixel.g(), pixel.b()])
            .collect();
        let file =
            std::io::BufWriter::new(std::fs::File::create(path).expect("creating the atlas"));
        let mut encoder = png::Encoder::new(file, WIDTH as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("writing the atlas's header");
        writer.write_image_data(&bytes).expect("writing the atlas");
    }
}

fn style(size: u32) -> FontdueRenderer<'static, Color> {
    let font = FontdueRenderer::new(
        FontdueRendererCtx::new_rc(),
        size,
        chrome::WHITE,
        chrome::FONTS,
    );
    text::style(&font, chrome::WHITE, size, chrome::FRAKTION)
}

fn mix(under: Rgb888, over: Rgb888, coverage: u8) -> Rgb888 {
    let (coverage, rest) = (u32::from(coverage), 255 - u32::from(coverage));
    let channel = |under: u8, over: u8| {
        ((u32::from(under) * rest + u32::from(over) * coverage + 127) / 255) as u8
    };
    Rgb888::new(
        channel(under.r(), over.r()),
        channel(under.g(), over.g()),
        channel(under.b(), over.b()),
    )
}

/// Text's coverage alone, so a label takes the page's colours at full depth rather than the
/// panel's.
struct Ink {
    coverage: Vec<u8>,
    width: usize,
    baseline: i32,
}

impl Ink {
    fn new(width: usize, size: u32) -> Self {
        let height = 2 * size as usize;
        Self {
            coverage: vec![0; width * height],
            width,
            baseline: (size + size / 2) as i32,
        }
    }
}

impl OriginDimensions for Ink {
    fn size(&self) -> Size {
        Size::new(self.width as u32, (self.coverage.len() / self.width) as u32)
    }
}

impl DrawTarget for Ink {
    type Color = Color;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Color>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(point, _) in pixels {
            self.blend_pixel(point, 255, chrome::WHITE);
        }
        Ok(())
    }
}

impl CoverageTarget for Ink {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], _color: Color) {
        let bounds = self.bounding_box();
        for (offset, &amount) in coverage.iter().enumerate() {
            let point = Point::new(x + offset as i32, y);
            if bounds.contains(point) {
                let index = point.y as usize * self.width + point.x as usize;
                self.coverage[index] = self.coverage[index].max(amount);
            }
        }
    }
}

const _: () = assert!(LCD_WIDTH == LCD_HEIGHT);
