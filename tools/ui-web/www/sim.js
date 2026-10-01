"use strict";

// Runs the stage in octowhere-ui.wasm on the browser's clock: the pointer is the touchscreen,
// the keys on the bezel and the controls are the board's keys and readings.

const READING = {
  heading: 0, pitch: 1, roll: 2, calibration: 3, disturbed: 4, live: 5, upright: 6,
  zone: 7, clock: 8, supply: 9, fix: 10, spinning: 11, level: 12,
};
// The power controller's long press, and how long PWR is held to power the board on.
const LONG_MS = 1000;
const POWER_ON_MS = 512;
// The bezel's drawing units across, and the panel's place in them.
const BEZEL = 600;
// The keys: where each sits, clockwise from the top in degrees, and how far it spans.
const KEYS = { power: 45, boot: 135 };
const HALF_SPAN = 9;

const canvas = document.getElementById("panel");
const context = canvas.getContext("2d");
const hint = document.getElementById("hint");
const off = document.getElementById("off");

const held = { power: false, boot: false, cover: false };
const pointer = { down: false, x: 0, y: 0, id: null };
let wasm;
let image;
let lastStatus = -1;

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

// The keys as arcs of the bezel, as the desktop simulator draws them, with a larger area to
// press and a fill that grows towards the long press.
for (const key of document.querySelectorAll(".key")) {
  const place = KEYS[key.dataset.key];
  key.querySelector(".hit").setAttribute("d", arc(226, 290, place, HALF_SPAN + 8));
  key.querySelector(".arc").setAttribute("d", arc(236, 244, place, HALF_SPAN));
  const angle = (place * Math.PI) / 180;
  const text = key.querySelector("text");
  text.setAttribute("x", 270 * Math.sin(angle));
  text.setAttribute("y", -270 * Math.cos(angle));
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
  for (const element of document.querySelectorAll(`[data-key="${name}"], [data-hold="${name}"]`)) {
    element.classList.toggle("held", now);
  }
}

// Keeps a pointer's events coming to `element` after it leaves. A pointer the browser did not
// make, as a test dispatches, cannot be captured, and works without.
function capture(element, event) {
  try {
    element.setPointerCapture(event.pointerId);
  } catch {}
}

function pressable(element, name) {
  element.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    capture(element, event);
    hold(name, `pointer${event.pointerId}`, true);
  });
  for (const type of ["pointerup", "pointercancel"]) {
    element.addEventListener(type, (event) => hold(name, `pointer${event.pointerId}`, false));
  }
  element.addEventListener("keydown", (event) => {
    if ((event.key === " " || event.key === "Enter") && !event.repeat) {
      event.preventDefault();
      hold(name, "focus", true);
    }
  });
  element.addEventListener("keyup", (event) => {
    if (event.key === " " || event.key === "Enter") hold(name, "focus", false);
  });
  element.addEventListener("blur", () => hold(name, "focus", false));
}

for (const key of document.querySelectorAll(".key")) pressable(key, key.dataset.key);
for (const button of document.querySelectorAll("[data-hold]")) {
  const progress = document.createElement("span");
  progress.className = "progress";
  button.append(progress);
  pressable(button, button.dataset.hold);
}

// How far a held key is towards its long press, or PWR towards powering on, on the bezel and
// the buttons.
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
    const fill = document.querySelector(`.key[data-key="${name}"] .fill`);
    fill.setAttribute("d", part > 0 ? arc(246, 250, KEYS[name] - HALF_SPAN + span, span) : "");
  }
}

// --- The touchscreen -----------------------------------------------------------------------

function place(event) {
  const box = canvas.getBoundingClientRect();
  const size = canvas.width;
  pointer.x = ((event.clientX - box.left) / box.width) * size;
  pointer.y = ((event.clientY - box.top) / box.height) * size;
}

canvas.addEventListener("pointerdown", (event) => {
  if (pointer.id !== null) return;
  event.preventDefault();
  capture(canvas, event);
  pointer.id = event.pointerId;
  pointer.down = true;
  place(event);
});
canvas.addEventListener("pointermove", (event) => {
  if (event.pointerId === pointer.id) place(event);
});
for (const type of ["pointerup", "pointercancel"]) {
  canvas.addEventListener(type, (event) => {
    if (event.pointerId !== pointer.id) return;
    pointer.id = null;
    pointer.down = false;
  });
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
  for (const button of group.querySelectorAll("button")) {
    button.setAttribute("role", "radio");
    button.addEventListener("click", () => {
      wasm.set(Number(group.dataset.reading), Number(button.dataset.value));
      sync();
    });
  }
}

// Puts the controls where the readings are, since the stage changes some of them itself and
// the keyboard changes the rest.
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
  for (const group of document.querySelectorAll(".segmented")) {
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
  "POWER OFF",
];
const RESTS = ["AWAKE", "DIMMING", "ALWAYS ON", "DARK"];

function showState(status) {
  let screen;
  if (status & 16) screen = "--";
  else if (status & 1) screen = "START-UP";
  else screen = VIEW_NAMES[(status >> 5) & 15] ?? (status & 8 ? "COMPASS" : "CLOCK");
  document.getElementById("state-screen").textContent = screen;
  document.getElementById("state-rest").textContent = status & 16 ? "POWERED OFF" : RESTS[(status >> 1) & 3];
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
    b: () => cycle("supply", [0, 1, 2]),
    "-": () => nudge("level", -step),
    _: () => nudge("level", -step),
    "=": () => nudge("level", step),
    "+": () => nudge("level", step),
    m: () => {
      const corners = document.getElementById("corners");
      corners.checked = !corners.checked;
      corners.dispatchEvent(new Event("change"));
    },
    p: screenshot,
  };
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
window.addEventListener("blur", () => {
  for (const name of Object.keys(sources)) {
    for (const source of [...sources[name]]) hold(name, source, false);
  }
});

// --- View ----------------------------------------------------------------------------------

document.getElementById("corners").addEventListener("change", (event) => {
  canvas.classList.toggle("corners", event.target.checked);
});

function screenshot() {
  canvas.toBlob((blob) => {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = "octowhere.png";
    link.click();
    URL.revokeObjectURL(link.href);
  });
}
document.getElementById("screenshot").addEventListener("click", screenshot);

// The panel at a whole number of device pixels per panel pixel, which keeps its pixels square
// and sharp, unless that would leave it much smaller than the room it has; then it fills the
// room, scaled smoothly.
function resize() {
  const main = document.querySelector("main");
  const ratio = window.devicePixelRatio || 1;
  const style = getComputedStyle(main);
  const inner = main.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  const columns = style.gridTemplateColumns.split(" ").length > 1;
  // Beside the controls, the device keeps room for them; above them it takes the width. The
  // stage pads it by 20 px a side, and on a wide screen the hint, the state strip and the
  // view controls stay in sight below it.
  const width = (columns ? inner - 24 - 360 : inner) - 40;
  const height = columns ? window.innerHeight - 290 : Infinity;
  const room = Math.min(width, height);
  const roomPanel = Math.max(120, (room * 466) / BEZEL);
  const scale = Math.floor((roomPanel * ratio) / 466);
  const whole = scale >= 1 && scale * 466 >= 0.8 * roomPanel * ratio;
  const css = whole ? (scale * 466) / ratio : roomPanel;
  document.documentElement.style.setProperty("--panel", `${css}px`);
  canvas.classList.toggle("pixelated", whole && scale >= 2);
  reserveHint();
}

// Sets the hint's height to the most lines any hint takes at the stage's width.
function reserveHint() {
  const shown = hint.textContent;
  const lineHeight = parseFloat(getComputedStyle(hint).lineHeight);
  const padding = parseFloat(getComputedStyle(hint).paddingTop) * 2;
  hint.style.minHeight = "0";
  let lines = 1;
  for (const text of [...Object.values(HINTS), ...VIEWS.filter(Boolean)]) {
    hint.textContent = text;
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
  clock: "The clock. Swipe sideways for the compass, or drag down from the top for settings.",
  compass: "The compass. Turn it with the heading control, or swipe back to the clock.",
};

// What shows over the faces, by the status's bits 5–8.
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
];

function showStatus(status) {
  if (status === lastStatus) return;
  lastStatus = status;
  const rest = (status >> 1) & 3;
  const view = VIEWS[(status >> 5) & 15];
  let text;
  if (status & 16) text = HINTS.powered;
  else if (status & 1) text = HINTS.starting;
  else if (rest === 1) text = HINTS.dimming;
  else if (rest === 2) text = HINTS.alwaysOn;
  else if (rest === 3) text = HINTS.dark;
  else if (view) text = view;
  else text = status & 8 ? HINTS.compass : HINTS.clock;
  hint.textContent = text;
  off.hidden = !(status & 16);
  showState(status);
}

// --- Running -------------------------------------------------------------------------------

function start(startUp) {
  wasm.start(startUp ? 1 : 0);
  const size = wasm.size();
  canvas.width = canvas.height = size;
  image = context.createImageData(size, size);
  lastStatus = -1;
  sync();
}

let lastSync = 0;

function frame(now) {
  const controls = (held.power ? 1 : 0) | (held.boot ? 2 : 0) | (held.cover ? 4 : 0);
  const result = wasm.step(now, Date.now() / 1000, pointer.x, pointer.y, pointer.down ? 1 : 0, controls);
  if (result & 1) {
    const size = canvas.width;
    image.data.set(new Uint8ClampedArray(wasm.memory.buffer, wasm.pixels(), size * size * 4));
    context.putImageData(image, 0, 0);
  }
  canvas.style.filter = `brightness(${wasm.light()})`;
  showStatus(wasm.status());
  showProgress();
  if (now - lastSync > 200) {
    sync();
    lastSync = now;
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

document.getElementById("replay").addEventListener("click", () => start(true));
document.getElementById("skip").addEventListener("click", () => start(false));
document.getElementById("power-on").addEventListener("click", () => start(true));

resize();
// The page's fonts change how the hints wrap once they arrive.
document.fonts?.ready.then(reserveHint);
load().catch((error) => {
  console.error(error);
  document.querySelector("#loading p").textContent = "COULD NOT LOAD";
  hint.textContent = String(error);
});
