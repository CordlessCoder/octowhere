# MARATHON-INSPIRED UI DESIGN LANGUAGE
## Portable design specification for expressive, functional interfaces

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

> **one meaning = one stable color role.**

Modes may have distinct colors, but state colors can cross modes. Keep normal, valid, attention, unknown and fault treatments consistent across the product. Check the role against each actual state before extending the palette.

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

Do not let one mode use flowing lines, another a 3×3 grid, another a conventional outline icon, and another a barcode.

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

A boot identity or other clearly ceremonial interval can carry a more expressive sequence. Specify it as a frame table at a declared rate, including brief on/off states, staggered arrivals and hard cuts. Keep operational transitions tied to their functional timing and state holds. A resting instrument should remain still unless motion itself conveys a live condition, such as charging.

---

## 9.2 Do not imitate trailer glitches in operational UI

The source aesthetic may contain dramatic motion, signal distortion, or aggressive transitions.

Those can inspire rhythm, but a working product should not use glitches where they could be confused with faults, flicker, input lag, or display corruption.

For bounded pattern motion, prefer deterministic, spatially stable fields. Move or replace a few marks at a time while the overall shape remains recognizable; account for the previous and next mark bounds when redrawing. Define grid, mark shapes, seed, clipping and exclusion areas in the implementation brief. A concept render with a surrogate hash is a visual target, not a claim of pixel identity or device performance.

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
- blue-on-black HUDs
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
3. Assign the screen or mode a strong identity color.
4. Divide the composition into hard rectangular zones.
5. Add one or two strong color slabs rather than many small accents.
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

> Design this interface using a **contemporary Marathon-inspired industrial graphic language**, without copying Marathon’s exact assets or layouts. Treat every screen as functional graphic art. Use a dark neutral base, hard rectilinear structure, bold modular slabs, aggressive scale contrast, compact technical microcopy, and sparse registration/fiducial details. Major modes should receive distinct saturated colors, used as large structural fields rather than tiny accents. Primary colored buttons should often use black knockout text and icons. Build all mode icons from one consistent low-resolution square-grid grammar with identical module thickness and optical weight. Favor asymmetry, stacked rails, square tiles, checker/pip motifs, hard borders, and dense but disciplined information clusters. Avoid glassmorphism, gradients, neon glow, generic cyberpunk HUD styling, rounded mobile-app cards, fake QR noise, and excessive diagonal shards. Large measurements and major actions must remain immediately legible. Decorative detail should reinforce hierarchy, identity, or system logic. Critical states and controls should become simpler and more visually forceful, not more ornamental. The result should feel like an authored industrial poster that also happens to be a real product interface.

---

# 17. Review checklist

Before approving a screen, ask:

- Does the composition itself have character, or is it a generic UI with decoration added later?
- Is there a clear dominant element?
- Are the major forms orthogonal and deliberate?
- Does color define meaningful system identity?
- Are solid color fields used boldly enough?
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

Keep source-video timecodes separate from an adapted animation's frame numbers. Record what a reference demonstrates, what was changed for the product and what was rejected. Label render explorations separately from approved direction and from firmware captures. If a new direction supersedes an older spec, name the superseding artifact and preserve the old one as history rather than letting two contradictory documents both read as current.

## 19.6 Use color on the selected value

On operational editors, put the saturated color behind the active value and use near-black text. Keep the neighboring values neutral. This makes a stepper, picker or slider feel like a member of an expressive screen family while the user's current choice remains the clearest element. Reserve attention and fault colors for those meanings; an ordinary editable choice should not look destructive.

Center the **visible glyph bounds** within a colored selection surface, not merely the font's nominal line box. Check the largest value and the longest label at native size; a number touching the slab edge weakens the selection's clarity. A screen family can have its own selection color from the established palette while keeping attention and live-data roles distinct. Treat a new role as a proposal until contrast and luminance are checked on the device.

## 19.7 Preserve data across reduced states

A low-power screen can remove ornament without discarding useful data. A small battery readout can survive a missing time source, and a known UTC clock can remain visible when no local zone is selected. Distinguish “unknown local conversion” from “unknown time” in words and digits. Tie any animated texture to an actual redraw event and verify battery update cadence, luminance and pixel movement on hardware.

## 19.8 Make simulations visibly simulated

If a device can replay a startup or demonstrate a component fault, distinguish that path from a live hardware test before selection and on the resulting fault presentation. Reuse the product's established selection and gesture grammar so a special utility path remains understandable. Do not let a demonstration overwrite the meaning of a stored diagnostic result.
