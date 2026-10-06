# MARATHON-INSPIRED UI DESIGN LANGUAGE
## Portable design specification for expressive, functional interfaces

**Updated: 6 October 2026.** Includes the accepted OCTOWHERE typography, spatial-member, runtime notification, messaging and consistency findings. Sections1–20 are portable principles; section21 records the current product profile. Product-specific choices do not automatically become requirements for other displays or projects.

**Purpose:** give a new design session enough context to immediately understand and reproduce the visual language we mean by the “Marathon UI vibe” without needing the original conversation.

This is **not** a request to copy Marathon’s exact screens, logos, typography, icons, or proprietary layouts. It is a reusable design language distilled from the contemporary Marathon aesthetic and adapted into an original system for real interfaces.

---

# 1. The core idea

The interface should feel like **industrial graphic design that happens to be interactive**.

Every screen should have the visual confidence of a poster, label system, technical placard, or aerospace control panel, while still behaving like a practical product UI.

The key tension is:

> **artful enough to feel authored, disciplined enough to feel operational.**

The design should never look like a generic dashboard with a sci-fi skin. The graphic composition itself must carry meaning.

A good screen should feel memorable even as a static image, but its strongest visual elements must also improve navigation, state recognition, hierarchy, or interaction.

---

# 2. Non-negotiable principles

## 2.1 Orthogonal first

The visual language is primarily **rectilinear**.

Use:

- rectangles
- square modules
- vertical and horizontal divisions
- stacked information blocks
- rails
- grids
- hard crop lines
- large flat fields
- blunt borders
- corner brackets
- box-within-box structures

Avoid making large diagonal shards, triangular cuts, or slanted panels the dominant motif.

A tiny slash, diagonal indicator, stepped edge, or angled accent can be useful, but the underlying grammar should still feel like it came from a grid.

**Rule:** if the layout would collapse without its diagonal decoration, it is probably the wrong direction.

---

## 2.2 Composition over decoration

Do not “decorate” a conventional UI after the fact.

Instead, compose the screen itself graphically:

- one block may dominate 40% of the screen
- another may be deliberately compressed
- a label may sit on a vertical edge
- an action may become a full-width color slab
- a value may be oversized enough to become the visual center of gravity
- supporting metadata may form a dense technical strip

The arrangement should create visual rhythm before ornamental elements are added.

The screen is the artwork. Borders, marks, icons, and microcopy are secondary.

---

## 2.3 Strong hierarchy, not equal boxes

Do not give every piece of information the same visual weight.

Typical hierarchy:

1. **Primary state or primary measurement**
2. **Primary action**
3. **Mode / identity**
4. **Secondary values**
5. **Process state / phase / status**
6. **Metadata and microcopy**

A large number can occupy a dramatic amount of space. Secondary values may be tightly packed into a technical strip.

The design works because large and small elements coexist aggressively.

---

## 2.4 Every screen has a visual identity

Screens in the same product should share one system, but they should not all be identical templates with swapped text.

A Home screen can feel calm and spacious.
A Running screen can feel kinetic and instrument-like.
A Fault screen can become stark and confrontational.
A Results screen can feel analytical and structured.

The design system should create **family resemblance without monotony**.

---

# 3. Color philosophy

## 3.1 Dark neutral base

Start with a dark neutral field. On an emissive display, pure black can be the right ground: an unlit pixel gives the colored slabs and small marks unusually sharp contrast. Choose the base for the actual panel and viewing conditions.

Typical base roles:

- near-black background
- slightly raised charcoal panels
- muted structural gray
- pale gray / off-white primary text

The neutral base gives saturated colors enough visual force.

---

## 3.2 Use saturated colors as structural material

Color is not merely an accent.

It can define:

- a mode
- a screen family
- a major action
- a status block
- a header
- a navigation slab
- a section boundary

Use **large, unapologetic color fields**.

A colored button should often be completely filled.
A mode header can be a full slab.
A small icon can live inside a saturated square tile.

Do not reduce every color to a tiny 2 px underline.

---

## 3.3 Black knockout text on bright color

One of the strongest recurring interaction patterns is:

> **bright solid rectangle + black text/icon**

This gives the UI a print/wayfinding quality rather than the usual glowing-digital look.

Use this for:

- primary actions
- selected modes
- prominent navigation
- mode tiles
- high-confidence controls

When the fill is saturated enough, black text often looks more characteristic than white text.

---

## 3.4 Give functional domains distinct colors

When a product has multiple major modes, give each one a strong, stable identity color.

Example family:

- acid lime / chartreuse
- hot magenta
- electric cyan
- signal orange
- fault red

The exact palette may change by project, but the principle is:

> **a stable semantic role, with explicit contextual exceptions.**

Modes may have distinct colors, but state colors can cross modes. Keep normal, valid, attention, unknown and fault treatments consistent across the product. Check the role against each actual state before extending the palette.

A structural backdrop can make a foreground color inappropriate even when its meaning is sound. Document a contextual substitution and retain its words/glyphs/data source. OCTOWHERE uses violet direct-contact text over its blue spatial grid; blue keeps its established live-reading role elsewhere. A blue grid is structural orientation, not a claim that every node is healthy. Do not require a unique saturated color for every page. Neutral reading surfaces can share one background while their content and hierarchy distinguish them.

---

## 3.5 Reserve danger colors

Red should remain meaningful.

Use it for:

- faults
- destructive / emergency actions
- Stop
- hard blocks
- safety-critical conditions

Avoid using red merely because a designer wants “one more accent.”

Choose the exact danger/attention split per product. In OCTOWHERE, red specifically means a genuine fault; removal, decline, return and other consequential actions use orange plus explicit consequence/confirmation. A destructive action is not automatically evidence of a hardware fault.

---

# 4. Typography

## 4.1 Typography should feel engineered, not futuristic-for-its-own-sake

Use a family with:

- strong grotesk / neo-grotesk structure
- clear uppercase forms
- compact or moderately condensed proportions
- reliable numeric rendering
- good small-size legibility

The feeling should be closer to industrial labeling, signage, sports branding, or technical documentation than to holographic science fiction.

---

## 4.2 Use dramatic scale contrast

Typical roles:

- **huge number / state:** dominant visual element
- **screen title:** bold, compact
- **button text:** heavy and blunt
- **field labels:** uppercase, smaller
- **microcopy:** compact, technical, secondary

A good Marathon-inspired screen often has a much larger difference between its largest and smallest type than a conventional product UI.

Preserve the typeface's native proportions unless distortion is an explicit, reviewed part of the composition. A naturally wide face, centered against its supporting marks, reads more intentionally than a headline stretched to fill empty space.

---

## 4.3 Microtype is texture, but must remain meaningful

Small technical labels can create the distinctive dense “machine” character.

Use microcopy for:

- IDs
- timestamps
- state codes
- channel names
- units
- revision strings
- mode labels
- system annotations

But do not fill space with fake hexadecimal strings or meaningless jargon.

The best microtype feels like the interface has an internal language because it actually does.

---

## 4.4 Numbers deserve special treatment

Measurements, timers, percentages, and gains should be visually stable.

Prefer:

- tabular numerals where available
- consistent decimal precision
- strong alignment
- fixed unit placement
- large readable values

Numbers can become major compositional elements rather than merely text fields.

Assign fonts by role rather than asking one display face to do everything: distinctive measurement numerals, compact labels, readable prose and stable technical data may use different faces. Match related values such as seconds to the main digits. Fit titles using actual ink width; reduce size uniformly within a defined range, then wrap/shorten the navigation label while retaining full identity in content. Never horizontally compress a font to make a title fit. Align button text and registration marks to measured drawn bounds, not nominal ascenders or advance width.

---

# 5. Icon and sigil language

## 5.1 Icons are symbols, not illustrations

Use compact, geometric, low-resolution marks.

They should feel like:

- aerospace fiducials
- machine-readable registration marks
- pixel sigils
- technical glyphs
- industrial pictograms

Not like:

- outline icon libraries
- emoji
- detailed illustrations
- arbitrary QR codes

---

## 5.2 One construction grammar

Every icon in a family must share:

- the same underlying grid
- the same module thickness
- the same optical weight
- the same bounding box logic
- the same rendering method

If one icon is built from a 3×3 block system, the others should also be built from that grammar.

Keep a shared construction discipline while allowing role-specific symbol vocabularies: a geometric status glyph, horizon cue and bounded barcode element need not have the same silhouette. Their stroke weight, grid, alignment and optical scale must still belong to the system. A reference barcode is not a QR code; reproduce its useful segment topology/motion only in the component it serves.

Consistency of **construction** matters more than literal resemblance.

Registration marks are also constructed elements: align each arm, inner square and fragment to the same corners and measured gaps. Their entrances and departures should respect that geometry rather than producing unrelated specks.

---

## 5.3 Favor low-resolution creativity

A 3×3, 5×5, or 7×7 grid can produce strong character because every occupied cell matters.

Good icons often resemble:

- gliders
- apertures
- calibration crosses
- stepped profiles
- alternating lattices
- directional marks

The designer should try to create an icon that feels both symbolic and slightly mysterious.

---

## 5.4 Icons should be reused exactly

The same mode symbol should appear unchanged in:

- the Home menu
- the mode header
- results
- logs
- mode selection

Do not redraw “similar” versions for different contexts.

---

# 6. Buttons and interactions

## 6.1 Buttons should feel like graphic objects

Primary buttons are not floating pills.

Prefer:

- rectangular slabs
- hard corners
- strong fill
- large label
- black knockout text on saturated fills
- optional icon tile
- explicit edge alignment

Avoid:

- soft glass buttons
- big corner radii
- gradients
- glow
- shadows used as the main affordance

---

## 6.2 Selected state should be unmistakable

A selected item may become:

- fully filled
- inverted
- enlarged
- bracketed
- moved into a stronger rail
- paired with a strong icon

Do not rely on a tiny border-color change.

---

## 6.3 Dangerous actions must remain visually obvious

The interface can be expressive without making critical controls clever or cryptic.

Emergency / Stop actions should be:

- large
- high-contrast
- consistently located when practical
- plainly labeled
- visually dominant when needed

The design language must never reduce discoverability for aesthetics.

Neutral actions can use a hard outline rather than a saturated slab. Keep their widths, gaps and ink centering consistent within a screen family. Visible graphic size and touch size are separate: a small navigation glyph still needs a practical hit area. Explain unavailable actions in words as well as muted styling. Confirmation belongs to meaningful consequences; removing an eligible history notification can be immediate without borrowing the treatment of a group-key change.

---

# 7. Layout grammar

## 7.1 Think in slabs and rails

Useful building blocks:

- mode slab
- title rail
- icon tile
- value field
- status strip
- segmented progress rail
- data card
- full-width action block
- footer rail
- side annotation

The screen should feel assembled from deliberate graphic modules.

---

## 7.2 Use asymmetry deliberately

Perfect symmetry is not required.

A strong composition may place:

- a large number left
- a compact label stack right
- a colored icon tile above
- a thin rail below

But asymmetry must be balanced by alignment and structure.

Random misalignment is not expressive.

Balance circular screens against their actual horizontal/vertical axes and glass edge. A centred title can be unbalanced by a single neighboring utility button; move secondary options into a deliberate footer or child page. If a scroll track occupies the right edge, centre its fixed arc around the rightmost horizontal point; its moving thumb can be asymmetric because it represents position. Let the broad horizontal middle carry wide information instead of forcing a tall stack into rotating rim labels.

---

## 7.3 Let whitespace be active

Not every area needs a border or label.

Empty dark space can create tension around:

- a huge value
- a small technical cluster
- a colored slab
- a warning

This makes dense areas feel intentional rather than cluttered.

---

## 7.4 Use repeated registration marks sparingly

Good secondary motifs include:

- tiny checker blocks
- square pips
- barcode-like separators
- short rule clusters
- corner brackets
- index numbers
- compact rails

These create system character.

They should behave like punctuation, not wallpaper.

---

# 8. Data visualization

## 8.1 Charts should look like instrumentation

Keep charts:

- compact
- high contrast
- low ornament
- integrated into the surrounding composition

Prefer:

- one strong process line
- one restrained target line
- minimal axes
- sparse grid
- clear status annotation

Avoid making charts look like desktop analytics dashboards.

---

## 8.2 Distinguish meanings through more than color

Where possible use:

- solid vs dashed line
- thickness
- labels
- spacing
- shape

Color should reinforce meaning, not carry it alone.

---

# 9. Motion

## 9.1 Motion is functional punctuation

Use motion for:

- pressed state
- transition confirmation
- hold progress
- state change
- brief panel reveal

Avoid ambient animation that makes a control interface feel unstable.

A boot identity or other clearly ceremonial interval can carry a more expressive sequence. Specify it as a frame table at a declared rate, including brief on/off states, staggered arrivals and hard cuts. Keep operational transitions tied to their functional timing and state holds. A resting instrument keeps its primary reading and controls stable. A restrained, reviewed background cycle can breathe while its foreground remains still, provided it preserves legibility and fits display/power budgets. This is distinct from invented activity: charging segments or a countdown must still correspond to real charging/operation state. Reduce or stop redraw work according to dim/off/AOD policy.

---

## 9.2 Do not imitate trailer glitches in operational UI

The source aesthetic may contain dramatic motion, signal distortion, or aggressive transitions.

Those can inspire rhythm, but a working product should not use glitches where they could be confused with faults, flicker, input lag, or display corruption.

For bounded pattern motion, prefer deterministic, spatially stable fields. Use absolute phase/position rather than cumulative offsets. Coalesced progress and page changes must not restart a repeating motif or introduce jitter.

For segmented graphics, design joining and splitting as well as translation. Segments merely oscillating in place do not reproduce a reference whose groups merge and separate. The battery remains one readable charge-level bar even when it segments during charging; segment count/density follow its available extent.

Move or replace a few marks at a time while the overall shape remains recognizable; account for the previous and next mark bounds when redrawing. Define grid, mark shapes, seed, clipping and exclusion areas in the implementation brief. A concept render with a surrogate hash is a visual target, not a claim of pixel identity or device performance.

---

# 10. Screen-specific emotional tone

A strong system lets screen purpose influence composition.

## Home / mode selection

Should feel:

- poised
- spacious
- inviting
- strongly branded by mode color

Use bold mode slabs and large symbols.

## Running / active process

Should feel:

- kinetic
- instrument-like
- information-dense
- immediately controllable

Give current state and main measurement dominant weight.

## Results / diagnostics

Should feel:

- analytical
- resolved
- structured
- precise

Use tables, compact metric blocks, and clear completion state.

## Fault

Should feel:

- stark
- blunt
- difficult to ignore

Reduce decorative noise. Let red, state text, and required action dominate.

---

# 11. Density rules

The style can be visually dense, but hierarchy must prevent cognitive overload.

A useful rule:

> **dense metadata, sparse decisions.**

There may be many labels, rails, pips, and values, but the user should never wonder what the primary action or state is.

Avoid screens containing five equally loud colored boxes competing for attention.

---

# 12. What this style is NOT

Do not interpret “Marathon-inspired” as:

- generic cyberpunk
- neon glow everywhere
- transparent holographic panels
- generic blue-on-black HUD ornament with no information or orientation role
- random hex codes
- diagonal shards everywhere
- military green CRT imitation
- terminal-only UI
- maximalist noise
- rounded mobile-app cards
- generic game inventory grids
- QR-code decoration with no internal logic
- copying Marathon logos, proprietary iconography, or exact layouts

The useful DNA is **editorial industrial graphic design**, not “future computer screen.”

---

# 13. Practical design recipe

When designing a new screen:

1. Identify the single most important state, value, or action.
2. Give it disproportionate visual weight.
3. Choose color from established semantic roles; give a mode a distinct identity only where it improves recognition.
4. Divide the composition into hard rectangular zones.
5. Use a strong color slab where hierarchy calls for it; keep neutral reading/actions restrained rather than adding color to meet a quota.
6. Use black knockout text/icons inside saturated controls.
7. Add one consistent geometric sigil if the mode has one.
8. Compress secondary metrics into disciplined technical strips.
9. Add sparse system punctuation: pips, brackets, short rules, grid marks.
10. Remove any decoration that does not strengthen hierarchy or identity.
11. Confirm the screen still reads correctly in grayscale and without ornament.
12. Confirm critical controls remain obvious at native size.

---

# 14. Expressiveness dial

The same language can be tuned by project.

## Level 1 — restrained instrument

- mostly neutral surfaces
- small mode-color rails
- sparse iconography
- simple typography

## Level 2 — characteristic product

- full-width color actions
- large mode slabs
- obvious sigils
- stronger type contrast
- small technical motifs

## Level 3 — full Marathon-inspired expression

- aggressive scale contrast
- strong asymmetrical composition
- large color fields
- dense secondary microtype
- bold geometric symbols
- graphic rails and registration marks
- each screen composed almost like a poster

Even Level 3 must preserve usability.

---

# 15. Reusable token template

Use this as a starting point, not a fixed palette:

```text
BACKGROUND      near-black neutral
SURFACE         slightly lifted charcoal
BORDER          desaturated structural gray
TEXT            warm/cool off-white
TEXT_SECONDARY  muted blue-gray
MODE_A          acid lime
MODE_B          hot magenta
MODE_C          electric cyan
MODE_D          signal orange
FAULT           saturated red
INK_ON_COLOR    near-black
```

Recommended geometry:

```text
corner radius:          0–2 px
primary action:         solid fill
icon system:            one fixed pixel grid
primary value:          oversized
microcopy:              uppercase / compact
layout:                 orthogonal modules
motion:                 purposeful only
shadows/glow:           usually none
```

---

# 16. Portable prompt for another design session

Copy this section directly into a future design conversation:

> Design this interface using a **contemporary Marathon-inspired industrial graphic language**, without copying Marathon’s exact assets or layouts. Treat every screen as functional graphic art. Use a dark neutral base, hard rectilinear structure, bold modular slabs, aggressive scale contrast, compact technical microcopy, and sparse registration/fiducial details. Use saturated colors as meaningful structural fields where needed, with stable status roles and explicitly documented contextual exceptions. Neutral reading pages can share one quiet background. Primary colored buttons should often use black knockout text and icons. Keep symbols in a coherent geometric construction system, with matched stroke/grid/alignment rules and optical weight; role-specific status glyphs and bounded barcode elements can have distinct silhouettes. Favor asymmetry, stacked rails, square tiles, checker/pip motifs, hard borders, and dense but disciplined information clusters. Avoid glassmorphism, gradients, neon glow, generic cyberpunk HUD styling, rounded mobile-app cards, fake QR noise, and excessive diagonal shards. Large measurements and major actions must remain immediately legible. Decorative detail should reinforce hierarchy, identity, or system logic. Critical states and controls should become simpler and more visually forceful, not more ornamental. The result should feel like an authored industrial poster that also happens to be a real product interface.

---

# 17. Review checklist

Before approving a screen, ask:

- Does the composition itself have character, or is it a generic UI with decoration added later?
- Is there a clear dominant element?
- Are the major forms orthogonal and deliberate?
- Does color define meaningful system identity?
- Are color fields strong where hierarchy needs them, and restrained where reading needs quiet?
- Do primary colored controls use strong contrast, often black knockout text?
- Do all icons clearly belong to one construction family?
- Is microcopy meaningful rather than decorative nonsense?
- Are there enough small technical details to create texture without clutter?
- Are diagonals and odd shapes accents rather than the core grammar?
- Does each screen have an individual visual personality while remaining in the same system?
- Is the main action/state obvious within one second?
- Would the design still make sense without color?
- Are critical controls more obvious, not less, because of the visual style?
- Does it feel like **industrial editorial design**, rather than “a sci-fi dashboard”? 

If most answers are yes, the design is in the right family.

---

# 18. One-sentence definition

> **The Marathon-inspired UI vibe is bold industrial editorial design: dark structured surfaces, hard modular geometry, saturated functional color, black knockout controls, extreme typographic hierarchy, compact technical microcopy, and a coherent pixel-sigil language—expressive enough to feel like art, disciplined enough to remain a real instrument.**

---

# 19. Lessons from a round display system (September 2026)

These are portable observations from developing a clock, compass, settings panel, always-on state, diagnostics and startup identity together. The OCTOWHERE-specific geometry, colors, approvals and timings live in that project's own specs.

## 19.1 Review a family at the same scale

Put the current screens side by side at native pixel size, including valid, missing, attention and fault states. Compare the hierarchy, type sizes, ring or edge treatment, symbol construction, meaningful color, microtext density and empty space. A ceremonial identity can be dense while the always-on face is sparse; coherence comes from repeated rules, not equal amounts of ornament.

## 19.2 Protect the primary reading

Place texture beneath content, with solid ground under important values and a small dark clearance around elements that sit on a pattern. Reserve an exclusion band around dense typography. On a circular panel, check every ring, hatch, label and gesture affordance against the actual mask; a rectangular screenshot can hide edge clipping.

## 19.3 Treat patterns as primitives

Specify a scatter field by its grid, mark vocabulary, seeded selection, shape envelope, clipping, density and update cadence. One shared primitive can create a quiet static instrument background or a brief identity plume by changing those parameters. The same shape language should remain visible in both. Test update cost on hardware before promising a frame rate.

## 19.4 Carry a motif only where it helps

An index grid, short registration mark, sparse scatter or rotated label may connect an otherwise austere settings panel to an expressive clock. Add it only after checking the touch target, active selection, data readout and scrolling states. Never transplant a full title composition onto an operational screen. A fault screen benefits from even fewer competing motifs.

## 19.5 Make reference and approval traceable

Keep source-video timecodes separate from an adapted animation's frame numbers. Distinguish a real-time excerpt, an orientation study and compressed state snapshots; a three-second comparison of a multi-minute task is not its implementation timing. Record what a reference demonstrates, what was changed for the product and what was rejected. Label render explorations separately from approved direction and from firmware captures. If a new direction supersedes an older spec, name the superseding artifact and preserve the old one as history rather than letting two contradictory documents both read as current.

## 19.6 Use color on the selected value

On operational editors, put the saturated color behind the active value and use near-black text. Keep the neighboring values neutral. This makes a stepper, picker or slider feel like a member of an expressive screen family while the user's current choice remains the clearest element. Reserve attention and fault colors for those meanings; an ordinary editable choice should not look destructive.

Center the **visible glyph bounds** within a colored selection surface, not merely the font's nominal line box. Check the largest value and the longest label at native size; a number touching the slab edge weakens the selection's clarity. A screen family can have its own selection color from the established palette while keeping attention and live-data roles distinct. Treat a new role as a proposal until contrast and luminance are checked on the device.

## 19.7 Preserve data across reduced states

A low-power screen can remove ornament without discarding useful data. A small battery readout can survive a missing time source, and a known UTC clock can remain visible when no local zone is selected. Distinguish “unknown local conversion” from “unknown time” in words and digits. Tie any animated texture to an actual redraw event and verify battery update cadence, luminance and pixel movement on hardware.

## 19.8 Make simulations visibly simulated

If a device can replay a startup or demonstrate a component fault, distinguish that path from a live hardware test before selection and on the resulting fault presentation. Reuse the product's established selection and gesture grammar so a special utility path remains understandable. Do not let a demonstration overwrite the meaning of a stored diagnostic result.


---

# 20. Lessons from spatial and runtime screens (October 2026)

## 20.1 Continuous background, protected foreground

A dark patterned field can remain continuous beneath an operational composition. Black card-shaped knockouts that are larger than the content can divide the surface unnecessarily. Reserve only the space needed for important ink, icons and dense reading; use measured exclusions rather than an unrelated large panel.

A spatial grid is particularly useful when it supplies orientation feedback. Its rotation, node placement and labels must derive from the same valid heading reference. A bearing ring is not a map: uniform node radius describes angle, not equal distance. A muted structural grid can coexist with green/lime selection and purple information without becoming the foreground's dominant element.

Keep that backdrop in empty and unavailable states. Missing position data should change the content and claims, not make the whole screen appear to belong to a different system. Valid heading and valid position are independent capabilities; if heading is unavailable, label the north-up fallback explicitly.

## 20.2 Shape carries status; color carries emphasis

A square node can carry a status glyph inside its frame. A filled square and an hourglass distinguish recent and older position observations even without color. Selection can highlight the frame/glyph/label while the actual status remains readable. Keep the background visible inside the frame; occlude conflicting ring strokes locally rather than making a large opaque card.

Attach compact identifiers/ages near nodes, with full details in one stable central reading area. Tangential labels follow the same angle as their nodes; central content stays upright. Sparse examples are not validation of dense groups: coincident bearings, long names and maximum node counts need explicit access/disambiguation behavior.

## 20.3 Typography roles across a family

Use display typography for identity, measurement typography for large data, an engineered label face for compact headers and a readable body face for explanations/messages. Natural contrast between those roles is more useful than stretching every face into one shape.

Use sentence case for prose and preserve case in names. Keep technical IDs, timestamps and units compact and consistent. Multi-line explanations share a left reading column; titles, values and short single-line states may remain centred. Let body content wrap and scroll rather than shrinking it below the useful reading size. A new face needs testing at its actual role/size: a successful identity typeface is not automatically successful in diagnostics or settings.

## 20.4 Circular scrolling is a composition rule

Place the scrollbar on the glass edge as an arc, with a symmetric fixed track and proportional moving thumb. A subtle uniform reduction near the viewport edges can make items feel part of the circular surface while keeping central reading full size. Scale the entire foreground item together; retain glyph proportions, logical content height and practical touch targets.

Calculate the transform from each item's current screen-space centre. Do not bake different widths into fixed row identities or repeatedly resize an already-scaled bitmap. When content grows/shrinks or is removed, preserve the nearest surviving reading anchor before clamping to the new extent. Taller entries are acceptable when the list already scrolls.

## 20.5 Navigation and event layers

A shared runtime drawer can hold events and ongoing activity without replacing the user's underlying work. Distinguish root closure from child back navigation with consistent glyphs. Keep secondary options at a balanced fixed footer. Reserve horizontal root gestures for page switching; child screens and keyboards have their own navigation. Lock direction and preserve touch capture so one gesture cannot activate a row and navigate the underlying screen.

Runtime arrival may wake a resting display and show a temporary toast. The event remains accessible after the toast ends, and the previous editor/rest context is preserved. A small neutral edge indicator can communicate unread content across different faces and low-power states without recoloring their health state.

Notifications, actual content and ongoing conditions are separate objects. Reading is not completion; dismissing a history notification is not cancelling a task, changing membership or deleting a message. Preserve active conditions, ongoing operations and outstanding responses until settled. Individual dismissal removes only the chosen eligible record and updates unread aggregates. Bulk clear can have stricter eligibility than individual dismissal.

## 20.6 Progress and status require evidence

Show elapsed/remaining time only when a defensible duration/deadline exists; otherwise show the actual phase or attempt count. A time-window rail is not a percentage of members found or acknowledgements received. Routine progress updates the same record quietly, rather than creating new cards or repeated wake/toast events.

Keep independent facts independent: receiver response versus satellite fix, observed position age versus direct-contact age, packet transmission versus relay versus private acknowledgement, local key switch versus all-peer completion. Copy, icons, color and read state must follow those distinctions. An empty result is not automatically a fault. An unavailable value is not zero or NOW.

## 20.7 Pattern brightness and coverage can move together

A halftone field can modulate both how many marks are visible and which dark tone each mark uses. Fixed point identity makes a density cycle feel continuous; rerolling the seed makes it sparkle or jitter. Reserve the brightest dark tier for denser accents and keep the prose field quieter.

Use one absolute time phase across navigation and derive motion from absolute state. Text/controls remain protected by stable ink exclusions and correct dirty redraws. A high-resolution preview bitmap is not a mandate for large embedded allocations: preserve the product's native primitive/hash and implement the intended envelope, tone relationship and clipping efficiently.

## 20.8 Review, acceptance and handoff

Build an atlas of all screen families at one native scale, including empty, missing, active and fault states. Resolve consistency through shared rules, while retaining deliberate composition exceptions. Separate approved pixel targets from historical alternatives, supplied firmware captures and animation poses.

A handoff should name every new screen, route, source of displayed truth, unavailable action, animation interval and implementation contract. Distinguish accepted visual/interaction choices from unshown engineering parameters. Include native references and reproducible source; identify overrides explicitly so an older footer, texture or layout cannot silently become current again.

---

## 20.9 Preserve identity at commitment

Keep the full supported name and a compact device discriminator visible while composing or confirming. A name, short ID or displayed fingerprint is presentation; capture the immutable command identity and revalidate it at commitment. Renaming, removal and ID reuse must not silently change the destination. Unavailable actions need a clear reason while readable history remains accessible.

## 20.10 Aggregation represents extent, not a fictitious object

When a spatial display becomes crowded, represent a group as its occupied interval or region rather than an ordinary person marker at an average point. Preserve the selected real object's exact position. Summaries report count and observed range; unknown evidence stays explicit. Measure rotated ink, reserve selected content, and handle circular wrap and coincident observations without inventing spread. Changes in grouping must not change selection identity.

## 20.11 Reading and capacity are evidence contracts

Unread state follows exposure to actual content, not entry into its parent screen. Oversized content needs accumulated fully visible coverage and a bounded tracking policy. Occluded or clipped fragments do not qualify. Ordinary history capacity is separate from authoritative live/actionable state; eviction must not remove an active action's only route or discard unread underlying messages. Counts and scroll extent describe the actual view, including derived protected state.

## 20.12 Retire obsolete controls cleanly

When a transport or workflow changes, remove obsolete action affordances and reclaim reading space. Do not replace them with invented activity, a permanent spinner or latency promises. Historical results retain their original meaning. Race-sensitive links bind to the exact request and reuse existing outcome/unavailable treatment if it changes before activation.

# 21. OCTOWHERE application profile — current 6 October 2026

These are product-specific applications of the language above. Use them when continuing OCTOWHERE; measure afresh for another product. They consolidate the current mesh/runtime handoff without reopening settled clock, compass, settings, AOD, identity, startup, charging or fault work.

## 21.1 Font roles and measured geometry

| Role | Current face / treatment |
| --- | --- |
| Identity title | Maratype, native proportions; identity-only. Subtitle spans the measured title width, with symbol/icon height matched to version/OK text. |
| Clock/AOD digits and seconds | KH Interference Bold; seconds matched to the main measurement face. |
| Compact operational labels | KH Interference Bold; clock band pairs KH labels with sans data. |
| Screen titles | Shapiro; new drawer/child titles nominal32px at Y48, uniformly fit32→26px within274px measured ink width. |
| Reading/explanation text | Sans Light; sentence case, measured wrapping and scrolling. Messages18px, inbox previews15px in current targets. |
| Metadata, IDs, names and editor text | Fraktion Mono; case-preserving names and printable ASCII coverage. |
| Spatial title exception |24px at Y103, countY136, heading captionY156; horizontal content avoids rim labels. |
| Keyboard exception | Retain the validated full keyboard's geometry and reserved draft/title space; no blanket title/footer restyling. |

Centre controls using visible ink height, not a font line box. New neutral single footer176×32 atX145,Y410; paired134×32 atX93/239,Y375,12px gap; Mono12 labels. Minimum40px touch height clipped to glass. Active-dismissal reason layouts are documented exceptions. Multi-line child prose usesX88 as its common reading anchor.

Registration marks are1px pluses with equal arm lengths, placed on diagonals of the true drawn title bounds with equal per-axis clearance. The identity needle logo retains a smooth circular top and an octagonal opening centred on that top. Do not recenter it using the full pin/needle bounding box.

## 21.2 Color roles and contextual extensions

| Token / palette | Value | Meaning / boundary |
| --- | --- | --- |
| BLACK | #000000 | Unlit field and local knockout ink. |
| WHITE | #D2D3D6 | Neutral primary information, results, unread markers/arc and neutral progress; not a universal success flag. |
| GRAY | #888E98 | Secondary/inactive data, neutral outlines; unavailable actions also need a reason. |
| LIME | #C0FE04 | Identity/normal local clock; spatial orientation/selection and positive messaging controls. Selection does not prove freshness or health. |
| VIOLET | #B32BE5 | Settings editable selection; messaging context; direct-contact age on blue-grid member face as a documented contextual override. |
| BLUE | #409DE4 | Established actual live/valid readings and linked subsystem data elsewhere; not generic buttons. |
| ORANGE | #F1710D | Attention/degraded operation and deliberate consequential removal/decline/return. |
| RED | #F24723 | Genuine faults. No indoor fix, zero refresh contacts and removal are not hardware faults. |
| PURPLE | #5500E4 | Inherited texture/registration token, no status claim; current dark scatter uses the tiers below. |
| Quiet scatter tiers | #180A36 / #250C54 / #371374 | Density-linked texture; brightest tier restrained, no status meaning. |
| Spatial grid | #0C1521 lines / #4B628B intersections | North-aligned structural background, no health or distance-scale claim. |
| Scroll/progress track | #30343A | Secondary structural track. |

Animation-specific fault blue #001DFF and yellow stay within the settled cinematic fault treatment; they do not establish ordinary UI status tokens. Unused reference swatches stay unassigned until a concrete semantic use is reviewed. Carry meaning in words/glyphs as well as color.

## 21.3 Spatial face

Blue grid64px pitch,0.75px lines and2px intersections rotates from valid absolute true heading. Bearing ring radius213 around(233,233); screen-upright24px node frame,1px stroke, transparent interior. Recent glyph6px square; older glyph8×10px hourglass. Age labels at radius188 rotate tangentially with bearing-heading. Primary selected-member detail spans the horizontal centre; direct and position ages remain separate. Use labeled north-up if true heading is unavailable, and retain grid/ring structure in no-own-fix/no-positions states.

The implemented recent-position glyph threshold is5minutes. Broad centre tap steps selection; the complete member list remains accessible. Approved crowded treatment uses a bounded sector at radius205, neutral tangential count/age-range at radius174, and keeps the selected real node at its exact bearing. Its sector summary atY372 counts other peers only. Reserve selected/central ink, preserve6px label clearance and use circular minimal intervals. No centroid-person square, aggregate distance, invented spread or tiny rim touch target. Dynamic32-member extremes remain implementation validation, not an open aesthetic choice.

## 21.4 Drawer and activity

Events/Messages are horizontal drawer roots. Upward main-face drag opens Events; Events left-swipe opens Messages, right returns. Root down-chevron closes; child centred left-arrow returns one level. Keep independent scroll anchors and saved editor/face origin. Existing16px direction lock and160ms cubic ease-out settle remain; clip rectangular page surfaces once at the circular glass.

List viewportY122–409; bottom OPTIONS fixed. Arc radius225,−41°…+41° about the rightmost point;1px track,3px WHITE thumb. Thumb span max(8°,82°*min(1,287/contentHeight)). Row scaling is1−.08*smoothstep(q), q=clamp((abs(rowCentre−265.5)−65)/80,0,1). Whole foreground scales together; logical scroll/hit geometry stays usable. Typical event178px; longer content and genuine ongoing operations measure/grow. REFRESH DEVICES is retired under continuous listening; no new refresh activity is generated. Old settled refresh results can remain ordinary history until eligible dismissal/clear/eviction/restart.

Runtime arrivals wake and show the accepted5s untouched toast, then restore preceding rest state. Ordinary lower toast[98,312,368,412]; compact typing toast[140,22,326,65], with protected input. White unread arc on all faces/AOD: radius230,80°…100°,3px stroke. It represents unread content, not ongoing state.

Individual DISMISS is in detail, preserving horizontal root paging. Completed/resolved records are eligible; active conditions, running work and requests awaiting response remain protected. Update counts/arc, retain scroll anchor, remove only stable event identity. No confirmation dialog for eligible history dismissal. Actual messages/protocol state remain intact. The final detail footer uses neutral dismissal, paired with a needed destination where appropriate. Recheck eligibility at activation. The16-record ordinary history cap does not cap authoritative protected live/actionable views. Message unread content remains backed by the message store. Oversized messages accumulate fully visible body-line coverage,1second continuous dwell per line; fitting rows retain1second whole-row dwell. MARK ALL READ in Events applies to events only.

## 21.5 Background and motion

Awake halftone breath is10s, b(t)=.875+.125*cos(2πt/10):100%→75%→100%. Preview43% side lobes/9% quiet field on8px grid; measured text ink gets2px clearance. Brightest preview tier is roughly10% of lobe marks at peak. Firmware retains ui::scatter hash/mark vocabulary and tone law; surrogate point coordinates are not a new firmware contract. Share absolute phase and respect established dim/off/AOD redraw policy.

Ceremonial identity/fault motion retains its precise approved frame/time maps. Operational scrolling follows user movement; removal progress follows actual deadlines. Self-test automatically scrolls the seventh RADIO row over160ms from offset0to45, with position-responsive row widths and independent check progress. Charging uses the implemented absolute docking/splitting loop and centre build/wipe; older positive-gap re-spacing recipes are superseded. Identity upper scatter turns continuously from frame66to110. No new timing is introduced by this document.

## 21.6 Authority and implementation status

Latest supplied implementation baseline is master0eb78d8,5October2026. Settled device-tested identity, charging, fault, clock, AOD, compass, settings, keyboard and prior runtime/member/removal behavior stay in force. The accepted5October increment retires refresh, restores MEMBERS, preserves full recipient identity, introduces bounded sectors with exact selected bearings, qualifies oversized-message reads by visible lines, protects active/actionable state outside ordinary history and links refusal to the exact request. Its implementation response is still pending. Geometry stress tests, bounded memory/failure handling and race validation are engineering tasks, not unresolved design selections.

The continuation backup contains current documentation and source recipes, not an exhaustive historical atlas. Generated captures are regenerated; source videos are provided by Roman. Surrogate rendering never claims firmware pixel identity or hardware performance. Current detailed implementation requirements and remaining validation are in the continuation package's IMPLEMENTATION-STATE.md and docs/IMPLEMENTATION-CHECKS.md.
