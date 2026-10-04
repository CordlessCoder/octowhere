"use strict";

// Runs the stages in octowhere-ui.wasm on the browser's clock: the pointer is the touchscreen,
// the keys on the bezel and the controls are the board's keys and readings. With several
// devices, each runs the firmware's mesh node on a simulated radio between them, and the
// controls act on the one selected.

const READING = {
  heading: 0, pitch: 1, roll: 2, calibration: 3, disturbed: 4, live: 5, upright: 6,
  zone: 7, clock: 8, supply: 9, fix: 10, spinning: 11, level: 12, receiver: 13,
};
// The power controller's long press, and how long PWR is held to power the board on.
const LONG_MS = 1000;
const POWER_ON_MS = 512;
// The bezel's drawing units across, and the panel's place in them.
const BEZEL = 600;
// The keys: where each sits, clockwise from the top in degrees, and how far it spans.
const KEYS = { power: 45, boot: 135 };
const HALF_SPAN = 9;
// The radio's speeds, as the module numbers them, and the log lines the page keeps.
const SPEEDS = [1, 2, 5, 10, 30, 60, 120];
const LOG_LINES = 400;

const hint = document.getElementById("hint");
const unitsElement = document.getElementById("units");
const template = document.getElementById("unit-template");

const held = { power: false, boot: false, cover: false };
const pointer = { down: false, x: 0, y: 0, id: null };
let wasm;
let lastStatus = -1;
// Each device's elements: its glass, the damage view over it, its keys and its powered-off
// cover.
let units = [];
let selected = 0;
let several = false;

function arc(inner, outer, place, span) {
  const point = (radius, degrees) => {
    const angle = (degrees * Math.PI) / 180;
    return [radius * Math.sin(angle), -radius * Math.cos(angle)];
  };
  const [a, b] = [place - span, place + span];
  const [x0, y0] = point(inner, a);
  const [x1, y1] = point(outer, a);
  const [x2, y2] = point(outer, b);
  const [x3, y3] = point(inner, b);
  return `M${x0} ${y0} L${x1} ${y1} A${outer} ${outer} 0 0 1 ${x2} ${y2} L${x3} ${y3} A${inner} ${inner} 0 0 0 ${x0} ${y0}Z`;
}

// Runs `read` with device `n` selected in the module, for the exports that read one device.
function on(n, read) {
  if (n === selected) return read();
  wasm.select(n);
  try {
    return read();
  } finally {
    wasm.select(selected);
  }
}

// --- Holding the board's keys, from the bezel, the buttons or the keyboard -----------------

const sources = { power: new Set(), boot: new Set(), cover: new Set() };
const since = {};

function hold(name, source, down) {
  const set = sources[name];
  if (down) set.add(source); else set.delete(source);
  const now = set.size > 0;
  if (now === held[name]) return;
  held[name] = now;
  since[name] = now ? performance.now() : undefined;
  const bezel = units[selected]?.root;
  for (const element of document.querySelectorAll(`[data-key="${name}"], [data-hold="${name}"]`)) {
    const key = element.closest(".unit");
    element.classList.toggle("held", now && (!key || key === bezel));
  }
}

function releaseAll() {
  for (const name of Object.keys(sources)) {
    for (const source of [...sources[name]]) hold(name, source, false);
  }
}

// Keeps a pointer's events coming to `element` after it leaves. A pointer the browser did not
// make, as a test dispatches, cannot be captured, and works without.
function capture(element, event) {
  try {
    element.setPointerCapture(event.pointerId);
  } catch {}
}

function pressable(element, name, before) {
  element.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    before?.();
    capture(element, event);
    hold(name, `pointer${event.pointerId}`, true);
  });
  for (const type of ["pointerup", "pointercancel"]) {
    element.addEventListener(type, (event) => hold(name, `pointer${event.pointerId}`, false));
  }
  element.addEventListener("keydown", (event) => {
    if ((event.key === " " || event.key === "Enter") && !event.repeat) {
      event.preventDefault();
      before?.();
      hold(name, "focus", true);
    }
  });
  element.addEventListener("keyup", (event) => {
    if (event.key === " " || event.key === "Enter") hold(name, "focus", false);
  });
  element.addEventListener("blur", () => hold(name, "focus", false));
}

for (const button of document.querySelectorAll("[data-hold]")) {
  const progress = document.createElement("span");
  progress.className = "progress";
  button.append(progress);
  pressable(button, button.dataset.hold);
}

// How far a held key is towards its long press, or PWR towards powering on, on the selected
// device's bezel and the buttons.
function showProgress() {
  const off = lastStatus >= 0 && (lastStatus & 16) !== 0;
  for (const name of ["power", "boot"]) {
    const length = off && name === "power" ? POWER_ON_MS : LONG_MS;
    const part = held[name] ? Math.min(1, (performance.now() - since[name]) / length) : 0;
    for (const bar of document.querySelectorAll(`[data-hold="${name}"] .progress`)) {
      bar.style.width = `${part * 100}%`;
    }
    // Just outside the key, from its first edge clockwise, as far as the press has gone.
    const span = HALF_SPAN * part;
    units.forEach((unit, n) => {
      const fill = unit.root.querySelector(`.key[data-key="${name}"] .fill`);
      const shown = n === selected && part > 0;
      fill.setAttribute("d", shown ? arc(246, 250, KEYS[name] - HALF_SPAN + span, span) : "");
    });
  }
}

// --- The devices ---------------------------------------------------------------------------

// Gives device `n` the pointer and the controls.
function choose(n) {
  if (n === selected || n >= units.length) return;
  releaseAll();
  selected = n;
  wasm.focus(n);
  units.forEach((unit, i) => unit.root.classList.toggle("selected", i === n));
  lastStatus = -1;
  sync();
  showShift();
}

function makeUnit(n) {
  const root = template.content.firstElementChild.cloneNode(true);
  const canvas = root.querySelector(".panel");
  const damage = root.querySelector(".damage");
  const unit = {
    root,
    canvas,
    context: canvas.getContext("2d"),
    image: null,
    damage,
    damageContext: damage.getContext("2d"),
    off: root.querySelector(".off"),
    flushes: [],
    name: root.querySelector(".unit-name"),
    group: root.querySelector(".unit-group"),
  };
  root.querySelector(".unit-number").textContent = String(n + 1);
  canvas.setAttribute("aria-label", `Device ${n + 1}'s touchscreen`);
  // The keys as arcs of the bezel, as the desktop simulator draws them, with a larger area to
  // press and a fill that grows towards the long press.
  for (const key of root.querySelectorAll(".key")) {
    const place = KEYS[key.dataset.key];
    key.querySelector(".hit").setAttribute("d", arc(226, 290, place, HALF_SPAN + 8));
    key.querySelector(".arc").setAttribute("d", arc(236, 244, place, HALF_SPAN));
    const angle = (place * Math.PI) / 180;
    const text = key.querySelector("text");
    text.setAttribute("x", 270 * Math.sin(angle));
    text.setAttribute("y", -270 * Math.cos(angle));
    pressable(key, key.dataset.key, () => choose(n));
  }
  canvas.addEventListener("pointerdown", (event) => {
    if (pointer.id !== null) return;
    event.preventDefault();
    choose(n);
    capture(canvas, event);
    pointer.id = event.pointerId;
    pointer.down = true;
    place(event, canvas);
  });
  canvas.addEventListener("pointermove", (event) => {
    if (event.pointerId === pointer.id) place(event, canvas);
  });
  for (const type of ["pointerup", "pointercancel"]) {
    canvas.addEventListener(type, (event) => {
      if (event.pointerId !== pointer.id) return;
      pointer.id = null;
      pointer.down = false;
    });
  }
  root.querySelector(".power-on").addEventListener("click", () => {
    if (several) {
      choose(n);
      wasm.reset(1);
    } else {
      start(true);
    }
  });
  return unit;
}

// Lays out `count` devices, the first selected.
function layoutUnits(count) {
  releaseAll();
  units = [];
  for (let n = 0; n < count; n++) units.push(makeUnit(n));
  unitsElement.replaceChildren(...units.map((unit) => unit.root));
  several = count > 1;
  selected = 0;
  units[0].root.classList.add("selected");
  document.querySelector("main").classList.toggle("several", several);
  document.getElementById("air").hidden = !several;
  document.getElementById("scripted-note").hidden = several;
  document.getElementById("log-module").hidden = !several;
  logLines = [];
  showLog();
  buildMatrix(count);
  for (const button of document.querySelectorAll("#device-count button")) {
    button.setAttribute("aria-checked", String(Number(button.dataset.value) === count));
  }
  showCorners();
  showDamageView();
  resize();
}

function place(event, canvas) {
  const box = canvas.getBoundingClientRect();
  const size = canvas.width;
  pointer.x = ((event.clientX - box.left) / box.width) * size;
  pointer.y = ((event.clientY - box.top) / box.height) * size;
}

// --- The readings --------------------------------------------------------------------------

function set(name, value) {
  wasm.set(READING[name], Number(value));
}

const sliders = [
  ["heading", (v) => `${String(Math.round(v)).padStart(3, "0")}°`],
  ["pitch", (v) => `${v > 0 ? "+" : ""}${Math.round(v)}°`],
  ["roll", (v) => `${v > 0 ? "+" : ""}${Math.round(v)}°`],
  ["level", (v) => `${Math.round(v)}%`],
];
for (const [name, format] of sliders) {
  const input = document.getElementById(name);
  const out = document.getElementById(`${name}-out`);
  input.addEventListener("input", () => {
    set(name, input.value);
    out.textContent = format(Number(input.value));
    fill(input);
    showReadings();
  });
}

for (const box of document.querySelectorAll("input[type=checkbox][data-reading]")) {
  box.addEventListener("change", () => wasm.set(Number(box.dataset.reading), box.checked ? 1 : 0));
}
for (const select of document.querySelectorAll("select[data-reading]")) {
  select.addEventListener("change", () => wasm.set(Number(select.dataset.reading), Number(select.value)));
}
for (const group of document.querySelectorAll(".segmented")) {
  for (const button of group.querySelectorAll("button")) button.setAttribute("role", "radio");
}
for (const group of document.querySelectorAll(".segmented[data-reading]")) {
  for (const button of group.querySelectorAll("button")) {
    button.addEventListener("click", () => {
      wasm.set(Number(group.dataset.reading), Number(button.dataset.value));
      sync();
    });
  }
}

// Puts the controls where the selected device's readings are, since the stage changes some of
// them itself and the keyboard changes the rest.
function sync() {
  for (const [name, format] of sliders) {
    const input = document.getElementById(name);
    const value = wasm.get(READING[name]);
    if (document.activeElement !== input) input.value = value;
    document.getElementById(`${name}-out`).textContent = format(value);
    fill(input);
  }
  for (const box of document.querySelectorAll("input[type=checkbox][data-reading]")) {
    box.checked = wasm.get(Number(box.dataset.reading)) !== 0;
  }
  for (const select of document.querySelectorAll("select[data-reading]")) {
    select.value = String(wasm.get(Number(select.dataset.reading)));
  }
  for (const group of document.querySelectorAll(".segmented[data-reading]")) {
    const value = wasm.get(Number(group.dataset.reading));
    for (const button of group.querySelectorAll("button")) {
      button.setAttribute("aria-checked", String(Number(button.dataset.value) === value));
    }
  }
  // Without a battery there is no level to set.
  document.getElementById("level").disabled = wasm.get(READING.supply) === 2;
  showReadings();
}

// How far along its range a slider's value is, for its violet part.
function fill(input) {
  const part = (input.value - input.min) / (input.max - input.min);
  input.style.setProperty("--filled", `${part * 100}%`);
}

// --- The modules' symbols and readings, and the state strip -------------------------------

// The firmware's 5 × 5 glyphs, bit 4 the leftmost, from crates/octowhere-ui/src/ui.
const GLYPHS = {
  device: [0b00100, 0b00000, 0b01100, 0b00100, 0b01110],
  arrow: [0b00100, 0b01110, 0b10101, 0b00100, 0b00100],
  gnss: [0b00100, 0b01010, 0b10101, 0b01010, 0b00100],
  battery: [0b01110, 0b11111, 0b10001, 0b11111, 0b11111],
  // The page's own, in the same grammar: pixel shift's nine places, devices apart, and lines.
  shift: [0b10101, 0b00000, 0b10101, 0b00000, 0b10101],
  mesh: [0b11000, 0b11000, 0b00011, 0b11011, 0b11000],
  log: [0b11111, 0b00000, 0b11100, 0b00000, 0b11110],
};

// Drawn as the firmware's icon tiles are: a frame a quarter of the module, at least 2 px, round
// black, with the glyph's modules inside the padding.
function drawSigil(svg) {
  const [module, padding, frame] = [5, 3, 2];
  const side = 2 * padding + 5 * module;
  const rect = (x, y, size, fill) =>
    `<rect x="${x}" y="${y}" width="${size}" height="${size}" fill="${fill}"/>`;
  let shapes = rect(0, 0, side, "currentColor") + rect(frame, frame, side - 2 * frame, "#000");
  GLYPHS[svg.dataset.glyph].forEach((bits, row) => {
    for (let column = 0; column < 5; column++) {
      if (bits & (0b10000 >> column)) {
        shapes += rect(padding + column * module, padding + row * module, module, "currentColor");
      }
    }
  });
  svg.setAttribute("viewBox", `0 0 ${side} ${side}`);
  svg.innerHTML = shapes;
}
for (const svg of document.querySelectorAll(".sigil")) drawSigil(svg);

// The colour each role takes, as the firmware's screens give them.
const ROLE_COLOURS = { live: "var(--blue)", attention: "var(--orange)", unknown: "var(--gray)", neutral: "var(--white)" };

function showReading(id, sigil, text, role) {
  const reading = document.getElementById(id);
  reading.textContent = text;
  reading.dataset.role = role;
  if (sigil) document.getElementById(sigil).style.color = ROLE_COLOURS[role];
}

const CLOCKS = ["GNSS", "RTC", "STOPPED", "NO DATA"];

// What the readings make of each module, by the meanings the screens give their colours.
function showReadings() {
  const get = (name) => wasm.get(READING[name]);
  const heading = String(Math.round(get("heading")) % 360).padStart(3, "0");
  const calibration = get("calibration");
  if (!get("live")) showReading("compass-reading", "compass-sigil", "NO DATA", "unknown");
  else if (calibration < 100) {
    showReading("compass-reading", "compass-sigil", `${String(calibration).padStart(3, "0")}%`, "attention");
  } else if (get("upright")) showReading("compass-reading", "compass-sigil", "---", "unknown");
  else if (get("disturbed")) showReading("compass-reading", "compass-sigil", `${heading}°`, "attention");
  else showReading("compass-reading", "compass-sigil", `${heading}°`, "live");

  const clock = get("clock");
  const clockRole = ["live", "neutral", "attention", "unknown"][clock];
  showReading("time-reading", null, CLOCKS[clock], clockRole);
  document.getElementById("time-sigil").style.color = ROLE_COLOURS[get("fix") ? "live" : "unknown"];

  const supply = get("supply");
  const level = get("level");
  if (supply === 2) showReading("power-reading", "power-sigil", "NO BAT", "unknown");
  else {
    const text = `${level}%${supply === 0 ? " CHG" : ""}`;
    showReading("power-reading", "power-sigil", text, level <= 15 ? "attention" : "neutral");
  }
  document.getElementById("state-clock").textContent = CLOCKS[clock];
  document.getElementById("state-supply").textContent =
    supply === 2 ? "USB, NO BAT" : `${supply === 0 ? "USB" : "BAT"} ${level}%${supply === 0 ? " CHG" : ""}`;
}

// What shows and how it rests, by the status's bits, for the state strip.
const VIEW_NAMES = [
  null, "SETTINGS", "BRIGHTNESS", "DEVICE", "CLEAR", "TIME ZONE", "REPLAY", "TIMEOUT", "ALWAYS ON",
  "POWER OFF", "GROUP", "EVENTS", "MESSAGES", "EVENT", "MANAGE HISTORY", "CONVERSATION",
  "SEND TO", "DRAFT", "REVIEW",
];
const RESTS = ["AWAKE", "DIMMING", "ALWAYS ON", "DARK"];
const FACES = ["CLOCK", "COMPASS", "MEMBERS"];

function showState(status) {
  let screen;
  if (status & 16) screen = "--";
  else if (status & 1) screen = "START-UP";
  else screen = VIEW_NAMES[(status >> 5) & 31] ?? FACES[(status >> 10) & 3];
  document.getElementById("state-screen").textContent = screen;
  document.getElementById("state-rest").textContent = status & 16 ? "POWERED OFF" : RESTS[(status >> 1) & 3];
}

// --- The mesh: devices, speed, links and the nodes' log ------------------------------------

const deviceCount = document.getElementById("device-count");
for (const button of deviceCount.querySelectorAll("button")) {
  button.addEventListener("click", () => startDevices(Number(button.dataset.value), false));
}
document.getElementById("in-group").addEventListener("change", () => {
  if (several) startDevices(units.length, false);
});

const speedRail = document.querySelector("#speed .rail");
SPEEDS.forEach((speed, index) => {
  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute("role", "radio");
  button.textContent = `${speed}×`;
  button.title = speed === 1 ? "As fast as the page's clock" : `${speed} seconds of the radio's time a second`;
  button.addEventListener("click", () => {
    wasm.set_speed(index);
    showMesh();
  });
  speedRail.append(button);
});

const LINK_NAMES = ["out of reach", "in reach", "lossy"];
const matrix = document.getElementById("matrix");

function buildMatrix(count) {
  matrix.replaceChildren();
  matrix.style.gridTemplateColumns = `repeat(${count + 1}, 34px)`;
  matrix.append(document.createElement("span"));
  for (let to = 0; to < count; to++) {
    const label = document.createElement("span");
    label.textContent = String(to + 1);
    matrix.append(label);
  }
  for (let from = 0; from < count; from++) {
    const label = document.createElement("span");
    label.textContent = String(from + 1);
    matrix.append(label);
    for (let to = 0; to < count; to++) {
      const cell = document.createElement("button");
      cell.type = "button";
      if (from === to) {
        cell.className = "self";
        cell.disabled = true;
        cell.setAttribute("aria-hidden", "true");
      } else {
        cell.dataset.from = String(from);
        cell.dataset.to = String(to);
        cell.addEventListener("click", () => {
          wasm.cycle_link(from, to);
          showMesh();
        });
      }
      matrix.append(cell);
    }
  }
}

function readText() {
  return new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.text(), wasm.text_len()));
}

const clockText = (seconds) => {
  const s = Math.floor(seconds);
  return `${String(Math.floor(s / 3600)).padStart(2, "0")}:${String(Math.floor(s / 60) % 60).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
};

// Each device's group as its node holds it, the radio's speed and time, and the links.
function showMesh() {
  if (!several) {
    document.getElementById("mesh-reading").textContent = "SCRIPTED";
    return;
  }
  units.forEach((unit, n) => {
    const [name, group] = on(n, () => {
      wasm.describe();
      return readText().split("\n");
    });
    unit.name.textContent = name;
    unit.group.textContent = group;
  });
  const speed = wasm.speed();
  document.getElementById("mesh-reading").textContent =
    `${SPEEDS[speed]}×  ${clockText(wasm.air_seconds())}`;
  speedRail.querySelectorAll("button").forEach((button, index) => {
    button.setAttribute("aria-checked", String(index === speed));
  });
  for (const cell of matrix.querySelectorAll("button[data-from]")) {
    const state = wasm.link(Number(cell.dataset.from), Number(cell.dataset.to));
    cell.dataset.state = String(state);
    cell.setAttribute("aria-label", `Device ${Number(cell.dataset.from) + 1} to device ${Number(cell.dataset.to) + 1}: ${LINK_NAMES[state]}`);
  }
}

document.getElementById("reset-device").addEventListener("click", () => {
  wasm.reset(1);
  lastStatus = -1;
});

const log = document.getElementById("log");
const logSelected = document.getElementById("log-selected");
let logLines = [];
logSelected.addEventListener("change", showLog);

// Takes the lines the nodes logged since the last call.
function pullLog() {
  wasm.take_log();
  const text = readText();
  if (!text) return;
  for (const line of text.split("\n")) {
    if (!line) continue;
    const [device, level, message] = line.split("\t");
    logLines.push({ device: Number(device), level, message });
  }
  if (logLines.length > LOG_LINES) logLines = logLines.slice(-LOG_LINES);
  showLog();
}

function showLog() {
  const atEnd = log.scrollTop + log.clientHeight >= log.scrollHeight - 4;
  const shown = logLines.filter((line) => !logSelected.checked || line.device === selected + 1);
  log.replaceChildren(
    ...shown.map((line) => {
      const row = document.createElement("div");
      if (line.level === "WARN" || line.level === "ERROR") row.className = "warn";
      row.textContent = `${line.device}  ${line.message}`;
      return row;
    }),
  );
  document.getElementById("log-reading").textContent = `${shown.length} LINES`;
  if (atEnd) log.scrollTop = log.scrollHeight;
}

// --- Pixel shift and the damage view ---------------------------------------------------------

// The firmware's round of places (ui::shift::POSITIONS), in order.
const POSITIONS = [[0, 0], [3, 0], [2, 2], [0, 3], [-2, 2], [-3, 0], [-2, -2], [0, -3], [2, -2]];
const pad = document.getElementById("pad");
const padCells = [];
for (let row = -1; row <= 1; row++) {
  for (let column = -1; column <= 1; column++) {
    const index = POSITIONS.findIndex(([x, y]) => Math.sign(x) === column && Math.sign(y) === row);
    const [x, y] = POSITIONS[index];
    const cell = document.createElement("button");
    cell.type = "button";
    cell.setAttribute("role", "radio");
    cell.setAttribute("aria-label", `Hold at ${x}, ${y}`);
    cell.textContent = String(index);
    cell.addEventListener("click", () => {
      wasm.pin_shift(index);
      showShift();
    });
    pad.append(cell);
    padCells[index] = cell;
  }
}
const shiftAuto = document.getElementById("shift-auto");
shiftAuto.addEventListener("change", () => {
  if (shiftAuto.checked) wasm.pin_shift(-1);
  else wasm.pin_shift(wasm.shift_state() & 15);
  showShift();
});

const signed = (value) => (value > 0 ? `+${value}` : value < 0 ? `−${-value}` : "0");

function showShift() {
  const state = wasm.shift_state();
  const position = state & 15;
  const pinned = (state & 16) !== 0;
  padCells.forEach((cell, index) => cell.setAttribute("aria-checked", String(index === position)));
  pad.classList.toggle("pinned", pinned);
  shiftAuto.checked = !pinned;
  document.getElementById("display-reading").textContent = `${signed(wasm.shift_x())} ${signed(wasm.shift_y())}`;
  document.getElementById("display-sigil").style.color = pinned ? "var(--violet)" : "var(--white)";
  const seconds = Math.floor(wasm.shift_age() / 1000);
  const age = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  document.getElementById("shift-moved").textContent =
    `PLACE ${position}  /  ${pinned ? "HELD" : "MOVED"} ${age} AGO`;
}

// The flushes as the firmware's damage-debug outlines them, each fading over this long.
const DAMAGE_FADE_MS = 600;
const damageView = document.getElementById("damage-view");
let lastFlush = null;

function showDamageView() {
  for (const unit of units) {
    unit.damage.hidden = !damageView.checked;
    unit.flushes = [];
  }
}
damageView.addEventListener("change", showDamageView);

function recordFlush(unit, n, now) {
  const count = wasm.flushed_count();
  if (count === 0) return;
  const runs = new Int32Array(wasm.memory.buffer, wasm.flushed(), count * 4);
  const rects = [];
  let pixels = 0;
  for (let i = 0; i < count; i++) {
    const rect = Array.from(runs.subarray(i * 4, i * 4 + 4));
    rects.push(rect);
    pixels += rect[2] * rect[3];
  }
  if (n === selected) lastFlush = { count, pixels };
  if (damageView.checked) unit.flushes.push({ at: now, rects });
}

function drawDamage(unit, now) {
  if (!damageView.checked) return;
  const { damage, damageContext } = unit;
  unit.flushes = unit.flushes.filter((flush) => now - flush.at < DAMAGE_FADE_MS);
  damageContext.clearRect(0, 0, damage.width, damage.height);
  const full = (w, h) => w === damage.width && h === damage.height;
  unit.flushes.forEach((flush, index) => {
    const fade = 1 - (now - flush.at) / DAMAGE_FADE_MS;
    const latest = index === unit.flushes.length - 1;
    for (const [x, y, w, h] of flush.rects) {
      // Only the latest regions are filled, so a run of flushes does not wash the panel out.
      damageContext.strokeStyle = `rgba(255, 255, 255, ${fade})`;
      if (full(w, h)) {
        // The frame's edge is behind the round glass, so a full flush rings the glass instead.
        damageContext.lineWidth = 3;
        damageContext.beginPath();
        damageContext.arc(w / 2, h / 2, w / 2 - 2, 0, 2 * Math.PI);
        damageContext.stroke();
        continue;
      }
      if (latest) {
        damageContext.fillStyle = "rgba(255, 255, 255, 0.16)";
        damageContext.fillRect(x, y, w, h);
      }
      damageContext.lineWidth = 1;
      damageContext.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
    }
  });
}

function showDamageStats() {
  const stats = document.getElementById("damage-stats");
  if (!lastFlush) return;
  const regions = lastFlush.count === 1 ? "1 REGION" : `${lastFlush.count} REGIONS`;
  stats.textContent = `LAST FLUSH  /  ${regions}  /  ${lastFlush.pixels.toLocaleString("en")} PX`;
}

// --- The keyboard, as the desktop simulator has it -----------------------------------------

function cycle(name, values) {
  const at = values.indexOf(wasm.get(READING[name]));
  set(name, values[(at + 1) % values.length]);
}

function toggle(name) {
  set(name, wasm.get(READING[name]) ? 0 : 1);
}

const HOLD_KEYS = { k: "power", o: "boot", h: "cover" };

document.addEventListener("keydown", (event) => {
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) return;
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const key = event.key.toLowerCase();
  if (HOLD_KEYS[key]) {
    hold(HOLD_KEYS[key], "keyboard", true);
    return;
  }
  const step = event.shiftKey ? 1 : 5;
  const nudge = (name, by) => set(name, wasm.get(READING[name]) + by);
  const speedBy = (by) => {
    wasm.set_speed(Math.max(0, Math.min(SPEEDS.length - 1, wasm.speed() + by)));
    showMesh();
  };
  const actions = {
    arrowleft: () => nudge("heading", -step),
    arrowright: () => nudge("heading", step),
    arrowup: () => nudge("pitch", step),
    arrowdown: () => nudge("pitch", -step),
    q: () => nudge("roll", -step),
    e: () => nudge("roll", step),
    " ": () => toggle("spinning"),
    c: () => cycle("calibration", [0, 54, 100]),
    d: () => toggle("disturbed"),
    t: () => toggle("upright"),
    l: () => toggle("live"),
    r: () => cycle("clock", [0, 1, 2, 3]),
    z: () => cycle("zone", [0, 1, 2, 3]),
    g: () => toggle("fix"),
    n: () => cycle("receiver", [0, 1, 2, 3, 4]),
    b: () => cycle("supply", [0, 1, 2]),
    "-": () => nudge("level", -step),
    _: () => nudge("level", -step),
    "=": () => nudge("level", step),
    "+": () => nudge("level", step),
    f: () => {
      damageView.checked = !damageView.checked;
      showDamageView();
    },
    m: () => {
      const corners = document.getElementById("corners");
      corners.checked = !corners.checked;
      showCorners();
    },
    p: screenshot,
  };
  if (several) {
    // A finger stays with the device it went down on.
    for (let n = 0; n < units.length; n++) actions[String(n + 1)] = () => pointer.down || choose(n);
    actions["["] = () => speedBy(-1);
    actions["]"] = () => speedBy(1);
    actions.x = () => {
      wasm.reset(1);
      lastStatus = -1;
    };
  }
  const action = actions[key];
  if (!action) return;
  event.preventDefault();
  action();
  sync();
});
document.addEventListener("keyup", (event) => {
  const name = HOLD_KEYS[event.key.toLowerCase()];
  if (name) hold(name, "keyboard", false);
});
window.addEventListener("blur", releaseAll);

// --- View ----------------------------------------------------------------------------------

const corners = document.getElementById("corners");

function showCorners() {
  for (const unit of units) {
    unit.canvas.classList.toggle("corners", corners.checked);
    unit.damage.classList.toggle("corners", corners.checked);
  }
}
corners.addEventListener("change", showCorners);

function screenshot() {
  units[selected].canvas.toBlob((blob) => {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = several ? `octowhere-${selected + 1}.png` : "octowhere.png";
    link.click();
    URL.revokeObjectURL(link.href);
  });
}
document.getElementById("screenshot").addEventListener("click", screenshot);

// The panels at a whole number of device pixels per panel pixel, which keeps their pixels
// square and sharp, unless that would leave them much smaller than the room they have; then
// they fill the room, scaled smoothly.
function resize() {
  const main = document.querySelector("main");
  const ratio = window.devicePixelRatio || 1;
  const style = getComputedStyle(main);
  const inner = main.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  const columns = style.gridTemplateColumns.split(" ").length > 1;
  const count = Math.max(1, units.length);
  // Beside the controls, one device keeps room for them; above them, the devices take the
  // width, side by side. The stage pads them by 20 px a side, and on a wide screen the hint,
  // the state strip and the view controls stay in sight below.
  let room;
  if (count > 1) {
    const width = (inner - 40 - (count - 1) * 20) / count;
    room = Math.min(width, window.innerHeight - 260);
  } else {
    const width = (columns ? inner - 24 - 360 : inner) - 40;
    const height = columns ? window.innerHeight - 210 : Infinity;
    room = Math.min(width, height);
  }
  const roomPanel = Math.max(120, (room * 466) / BEZEL);
  const scale = Math.floor((roomPanel * ratio) / 466);
  const whole = scale >= 1 && scale * 466 >= 0.8 * roomPanel * ratio;
  const css = whole ? (scale * 466) / ratio : roomPanel;
  document.documentElement.style.setProperty("--panel", `${css}px`);
  for (const unit of units) unit.canvas.classList.toggle("pixelated", whole && scale >= 2);
  reserveHint();
}

// Sets the hint's height to the most lines any hint takes at the stage's width.
function reserveHint() {
  const shown = hint.textContent;
  const lineHeight = parseFloat(getComputedStyle(hint).lineHeight);
  const padding = parseFloat(getComputedStyle(hint).paddingTop) * 2;
  hint.style.minHeight = "0";
  let lines = 1;
  for (const text of [...Object.values(HINTS), ...VIEWS.filter(Boolean), GROUP_SEVERAL]) {
    hint.textContent = several ? `Device 4. ${text}` : text;
    lines = Math.max(lines, Math.round((hint.offsetHeight - padding) / lineHeight));
  }
  hint.textContent = shown;
  hint.style.minHeight = "";
  document.documentElement.style.setProperty("--hint-lines", lines);
}
window.addEventListener("resize", resize);

// --- Status --------------------------------------------------------------------------------

const HINTS = {
  starting: "Starting up. Touch the screen to skip to the clock.",
  dimming: "Dimming after the screen timeout. Touch the screen to keep it awake.",
  alwaysOn: "Resting on the always-on face. Double-tap the screen to wake it.",
  dark: "The screen is off. Double-tap it to wake it.",
  powered: "Powered off. Hold PWR for half a second to power it on.",
  clock: "The clock. Swipe sideways for the compass, drag down from the top for settings, or up for events.",
  compass: "The compass. Turn it with the heading control, or swipe sideways for the clock or the members.",
  members: "Where the group's members are, on a ring that turns with the heading. Tap the middle for the next member.",
};

// What shows over the faces, by the status's bits 5–9.
const VIEWS = [
  null,
  "Settings. Tap a cell to change it, swipe sideways for the second page, or drag up to close.",
  "Brightness. Drag to try a level, then tap to keep it, or tap CANCEL.",
  "Device. Drag up for the details, the start-up replay and clearing the settings, or tap BACK.",
  "Clear settings. Slide the handle into the target to erase them, or tap CANCEL.",
  "Time zone. Drag to an offset and tap for its zones, then drag to a zone and tap to choose it.",
  "Replay. Drag to the start-up or a part's failure, then tap to play it, or tap BACK.",
  "Screen timeout. Drag to choose how long the screen stays awake, then tap to keep it.",
  "Always on. Drag to choose the face's level, or OFF to let the screen go dark, then tap.",
  "Power off. Slide the handle into the target to power off, or tap CANCEL. It cancels after 10 s.",
  "Group and name. The other device is simulated: after START it appears, shows the same code and confirms it.",
  "Events. Tap one for its detail, swipe left for messages, or drag down from the top to close.",
  "Messages. Tap a conversation, or NEW MESSAGE to write one. Swipe right for events.",
  "An event. DISMISS removes it once it has settled; the arrow at the top goes back.",
  "Manage history. Mark every event read, or clear the read ones that have settled.",
  "A conversation, newest first. A message counts read once it has shown whole for a second. WRITE answers.",
  "Send to. Choose the group or one member.",
  "A draft. Type with the keys on the screen, then REVIEW. CANCEL keeps it for later.",
  "Review. Read it through, then SEND, or EDIT to go back to it.",
];
const GROUP_SEVERAL =
  "Group and name. The other devices run real nodes: ADD on one and JOIN on another, then confirm the same code on both.";

function showStatus(status) {
  if (status === lastStatus) return;
  lastStatus = status;
  const rest = (status >> 1) & 3;
  const viewIndex = (status >> 5) & 31;
  const view = several && viewIndex === 10 ? GROUP_SEVERAL : VIEWS[viewIndex];
  let text;
  if (status & 16) text = HINTS.powered;
  else if (status & 1) text = HINTS.starting;
  else if (rest === 1) text = HINTS.dimming;
  else if (rest === 2) text = HINTS.alwaysOn;
  else if (rest === 3) text = HINTS.dark;
  else if (view) text = view;
  else text = [HINTS.clock, HINTS.compass, HINTS.members][(status >> 10) & 3];
  hint.textContent = several ? `Device ${selected + 1}. ${text}` : text;
  showState(status);
}

// --- Running -------------------------------------------------------------------------------

function prepare() {
  const size = wasm.size();
  for (const unit of units) {
    unit.canvas.width = unit.canvas.height = size;
    unit.damage.width = unit.damage.height = size;
    unit.image = unit.context.createImageData(size, size);
  }
  lastStatus = -1;
  sync();
  showMesh();
}

// One device on the scripted mesh.
function start(startUp) {
  wasm.start(startUp ? 1 : 0);
  if (units.length !== 1) layoutUnits(1);
  prepare();
}

// Several devices on the simulated radio, or one when `count` is 1.
function startDevices(count, startUp) {
  if (count <= 1) {
    start(startUp);
    return;
  }
  wasm.start_devices(count, document.getElementById("in-group").checked ? 1 : 0, Date.now() / 1000);
  layoutUnits(count);
  prepare();
}

let lastSync = 0;
let lastLog = 0;

function frame(now) {
  const controls = (held.power ? 1 : 0) | (held.boot ? 2 : 0) | (held.cover ? 4 : 0);
  const changed = wasm.step(now, Date.now() / 1000, pointer.x, pointer.y, pointer.down ? 1 : 0, controls);
  const size = wasm.size();
  units.forEach((unit, n) => {
    on(n, () => {
      if (changed & (1 << n)) {
        unit.image.data.set(new Uint8ClampedArray(wasm.memory.buffer, wasm.pixels(), size * size * 4));
        unit.context.putImageData(unit.image, 0, 0);
      }
      unit.canvas.style.filter = `brightness(${wasm.light()})`;
      unit.off.hidden = !(wasm.status() & 16);
      recordFlush(unit, n, now);
    });
    drawDamage(unit, now);
  });
  showStatus(wasm.status());
  showProgress();
  if (now - lastSync > 200) {
    sync();
    showShift();
    showDamageStats();
    showMesh();
    lastSync = now;
  }
  if (several && now - lastLog > 250) {
    pullLog();
    lastLog = now;
  }
  requestAnimationFrame(frame);
}

// A page opened from disk may not fetch, but may still run scripts beside it, so the build
// also writes the module into one as base64.
function embedded() {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = "octowhere-ui.wasm.js";
    script.onload = () => {
      const text = atob(window.OCTOWHERE_WASM);
      const bytes = new Uint8Array(text.length);
      for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i);
      resolve(bytes);
    };
    script.onerror = () => reject(new Error("octowhere-ui.wasm.js is missing; run build.sh"));
    document.head.append(script);
  });
}

// The module's size, which build.sh writes in. Unbuilt, it is not a number and the progress
// shows the bytes alone.
const MODULE_BYTES = Number("__MODULE_BYTES__");
const CELLS = 20;

const cells = document.getElementById("loading-cells");
for (let i = 0; i < CELLS; i++) cells.append(document.createElement("span"));

function showDownload(received) {
  const kb = (bytes) => `${Math.round(bytes / 1024).toLocaleString("en")} KB`;
  const known = Number.isFinite(MODULE_BYTES) && MODULE_BYTES > 0;
  const part = known ? Math.min(1, received / MODULE_BYTES) : 0;
  [...cells.children].forEach((cell, i) => cell.classList.toggle("on", i < Math.floor(part * CELLS)));
  document.getElementById("loading-bytes").textContent =
    known ? `${kb(received)} / ${kb(MODULE_BYTES)}` : kb(received);
}

// Fetches the module, counting its bytes as they arrive while the browser compiles them.
async function fetched(imports) {
  const response = await fetch("octowhere-ui.wasm");
  if (!response.ok) throw new Error(`octowhere-ui.wasm: ${response.status} ${response.statusText}`);
  const [counted, compiled] = response.body.tee();
  (async () => {
    const reader = counted.getReader();
    let received = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value.length;
      showDownload(received);
    }
  })();
  const module = new Response(compiled, { headers: { "Content-Type": "application/wasm" } });
  try {
    return await WebAssembly.instantiateStreaming(module, imports);
  } catch {
    // A browser without streaming compilation.
    const bytes = await (await fetch("octowhere-ui.wasm")).arrayBuffer();
    return WebAssembly.instantiate(bytes, imports);
  }
}

async function load() {
  const imports = {
    env: {
      console_error: (at, len) => {
        const text = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, at, len));
        console.error(text);
        hint.textContent = `The simulator stopped: ${text}`;
      },
    },
  };
  let instance;
  if (location.protocol === "file:") {
    ({ instance } = await WebAssembly.instantiate(await embedded(), imports));
  } else {
    ({ instance } = await fetched(imports));
  }
  wasm = instance.exports;
  document.getElementById("loading").hidden = true;
  start(true);
  requestAnimationFrame(frame);
}

// With several devices these reset the selected one, whose node keeps what it stored.
document.getElementById("replay").addEventListener("click", () => {
  if (several) {
    wasm.reset(1);
    lastStatus = -1;
  } else {
    start(true);
  }
});
document.getElementById("skip").addEventListener("click", () => {
  if (several) {
    wasm.reset(0);
    lastStatus = -1;
  } else {
    start(false);
  }
});

layoutUnits(1);
// The page's fonts change how the hints wrap once they arrive.
document.fonts?.ready.then(reserveHint);
load().catch((error) => {
  console.error(error);
  document.querySelector("#loading p").textContent = "COULD NOT LOAD";
  hint.textContent = String(error);
});
