# MARATHON-INSPIRED UI — CROSS-PROJECT HANDOFF
## Portable design and implementation doctrine for a new display, use case, and software stack

**Purpose:** hand this document to a new design/implementation session so it can immediately understand the visual language, design philosophy, research discipline, and implementation approach established in the previous project.

This document is intentionally **project-agnostic**. It does **not** carry forward toaster-oven dimensions, temperatures, recipes, safety rules, LVGL assumptions, ESP32 constraints, or previous screen structures. Those belonged to the old product.

What *does* carry forward is the design language and the way we work.

---

# 1. One-sentence definition

> **The target is industrial editorial graphic design made interactive: dark structured surfaces, saturated functional color, black knockout controls, extreme typographic hierarchy, compact technical metadata, and a coherent modular symbol language — expressive enough to feel authored, disciplined enough to behave like a real instrument.**

This is inspired by the **contemporary Marathon visual language**, but it should remain an original product interface rather than a copy of Marathon screens, logos, lore, exact icons, typography, or branded compositions.

---

# 2. The core tension

Every screen should satisfy two goals simultaneously:

1. **It should feel like graphic design.** A static screenshot should have composition, rhythm, contrast, identity, and visual authorship.
2. **It should behave like a real product.** State, action, hierarchy, feedback, danger, and data must be immediately understandable.

The correct result is not “a normal app with sci-fi decoration.”

The composition itself should communicate function.

> **Art comes from hierarchy, scale, color, structure, and rhythm — not from adding visual noise.**

---

# 3. Source hierarchy and research discipline

When studying Marathon or any future reference corpus, keep source types separate.

## 3.1 Highest-value references

Prefer, in roughly this order:

1. **Actual playable UI / real product screens** — best evidence for controls, navigation, selection, hierarchy, icon scale, density, and interaction treatment.
2. **First-hand designer process work** — excellent evidence for graphic grammar, typography, motion, registration marks, composition, and visual experimentation.
3. **Official trailers / marketing / promotional graphics** — useful for expressive art direction, but do not assume these represent playable interaction patterns.
4. **Third-party recreations / fan concepts** — inspiration only; never use them as proof of the source language.

Always label which category a reference belongs to.

## 3.2 Extract principles, do not trace assets

For every useful reference, ask:

- What is the underlying grid?
- What establishes hierarchy?
- How is color occupying space?
- How are selected and inactive states distinguished?
- How much of the personality comes from scale rather than ornament?
- What repeats consistently?
- What changes dramatically between screens?
- Which details are functional and which are promotional expression?

Translate these observations into reusable rules.

Do **not** solve a new product by reproducing an existing Marathon screen with different labels.

## 3.3 Separate evidence from interpretation

When presenting research, use this pattern:

> **Observed:** selected tabs/items use saturated full-fill rectangles.
>
> **Interpretation:** selection is communicated through color occupying physical area, not merely through a small underline.
>
> **Portable rule:** important selected states should become strong fields/slabs where the target display permits it.

This prevents visual folklore from becoming an undocumented rule.

## 3.4 Reference locations — where to start

The next design session should not rely on the phrase “Marathon vibe” from memory. Start with a small, classified reference pack. The following sources were useful during the previous session and were rechecked in September 2026.

### A. Official contemporary Marathon site — era and campaign identity

**URL:** `https://www.marathonthegame.com/`

Use for:

- current visual era and high-level art direction
- contemporary color confidence and graphic identity
- official screenshots, trailers, world presentation, and promotional composition
- confirming that the reference target is the current Bungie game rather than the original trilogy

Do **not** assume every promotional treatment is a playable UI pattern. Treat the site as an official art-direction source, not automatically as an interaction specification.

### B. Official PlayStation Marathon page and Runner guide — current game context

**URLs:**

- `https://www.playstation.com/en-us/games/marathon/`
- `https://www.playstation.com/en-us/games/marathon/runners-guide-marathon-tau-ceti-iv/`

Use for:

- official contemporary screenshots and current game presentation
- gameplay-system context around inventory, factions, loot, rarity, and equipment
- understanding which colors and symbols carry actual game meaning rather than purely promotional meaning
- corroborating current terminology and product context

The Runner guide is particularly useful when a screenshot is visually ambiguous: it explains inventory and item semantics such as rarity/category behavior. Do not treat editorial page layout as in-game UI.

### C. Bungie/Marathon Help community screenshot — actual playable UI evidence

**URL:** `https://help.marathonthegame.com/hc/en-us/community/posts/48859382912788-Compiler-Run-no-skin-for-my-buddy-and-myself-Screenshot`

This is a **player-supplied screenshot hosted on Marathon Help**, not an official press screenshot. It was one of the most useful references for the real playable interface.

Use for:

- actual inventory/loadout/vault UI
- rectangular item grids
- strong selected-tab treatment
- compact numeric/metadata typography
- solid filled selection/category cells
- bottom action/navigation bars
- the contrast between dark structural surfaces and bright selected states

Because it is a community upload, use it as firsthand evidence of the running game, not as a source of official design rationale.

### D. Michael Rigley / Antibody Marathon project — first-hand expressive process work

**URL:** `https://www.behance.net/gallery/205038723/MARATHON`

**Designer profile:** `https://www.behance.net/mrigley`

This is a first-hand designer portfolio describing screen graphics and animation created for the Marathon announcement trailer. It is **not playable UI**, which is exactly why it should be kept in a separate reference category.

Use for:

- oversized outlined/solid typography
- saturated full-frame color fields
- black/white modular blocks
- barcode/tick/registration details
- graphic process exploration
- controlled glitch/corruption language
- asymmetry created by scale, crop, and whitespace
- motion and promotional expressiveness

Do **not** copy trailer graphics directly into operational UI. Extract composition and motion principles, then test whether they survive real interaction requirements.

### E. Fan work — useful only as a negative/control reference

Example: `https://www.behance.net/gallery/213800383/FANMADE-Marathon-Title-Sequence`

This can be useful for identifying how people imitate the visual language, but it must never be cited as evidence of Bungie’s actual UI. Fan work is particularly useful for spotting common imitation drift: excessive cyberpunk noise, too many diagonals, or overuse of faux technical decoration.

### F. Legacy Marathon trilogy — explicitly out of scope unless the project asks for it

If older Marathon references are needed, Aleph One is a useful route into the original trilogy:

`https://alephone.lhowon.org/`

The visual language in this handoff targets the **contemporary Bungie Marathon**, not the 1990s interface. Do not mix the two accidentally.

## 3.5 How to preserve references for the next project

Do not depend on temporary chat image paths. Create a project-local reference index instead. A recommended structure is:

```text
references/
  marathon/
    README.md
    playable-ui/
    first-hand-designer-process/
    official-promo/
    fan-or-third-party-do-not-treat-as-canonical/
```

In `references/marathon/README.md`, record for every item:

- source URL
- source type: playable / first-hand process / official promo / third-party
- date captured or last checked
- what the reference demonstrates
- what it **does not** demonstrate
- any copyright/license restrictions
- filename of any locally saved screenshot, if the project is permitted to keep one

A useful entry looks like this:

```text
Source: Michael Rigley — MARATHON
URL: https://www.behance.net/gallery/205038723/MARATHON
Type: First-hand designer process / announcement-trailer graphics
Use for: scale contrast, color blocking, microtype, registration marks, motion
Do not infer: playable navigation or button behavior
Checked: 2026-09
```

When handing work to another model or designer, give them both this design-language document **and** that reference index. If local images are included, keep their provenance beside them. Do not strip an image out of context and let a future session guess whether it was gameplay, promotional work, or fan art.

## 3.6 Reference-reading rule

Use at least two different source categories before turning an observation into a core rule whenever possible. For example:

- playable UI can establish how selection actually works;
- first-hand trailer/process work can establish how aggressively typography and color may be pushed;
- the new product’s own usability constraints decide how those two influences are combined.

The target language comes from **the intersection of verified interaction patterns and expressive graphic systems**, not from copying any single screenshot.

---

# 4. Composition: orthogonal first

The visual grammar is fundamentally **rectilinear**.

Prefer:

- rectangles
- square modules
- stacked blocks
- rails
- grids
- vertical and horizontal separators
- box-within-box structures
- hard crops
- blunt borders
- apertures
- brackets
- full-width slabs
- offset columns
- asymmetric whitespace

Avoid making giant triangular cuts, shards, chevrons, or angled panels the core visual motif.

Small slashes, stepped edges, diagonal ticks, or clipped corners can appear as punctuation, but the interface should still feel as if it was constructed on a disciplined Cartesian grid.

**Test:** if removing the diagonals destroys the identity of the layout, the design has probably drifted into generic sci-fi styling.

---

# 5. Composition over decoration

Do not build a conventional card UI and decorate it afterward.

Instead, compose the entire screen graphically.

Examples:

- one primary value occupies 30–50% of the visual field
- an action becomes a full-width color slab
- a mode label becomes a colored architectural region rather than a small badge
- secondary values are compressed into a dense instrument strip
- an empty region is deliberately left empty to create tension
- metadata forms a thin rail along an edge
- a selected row consumes the whole width and becomes a strong color island

The layout should already feel intentional before adding checker marks, micro-labels, or decorative details.

---

# 6. Hierarchy: deliberately unequal

Do not give every value, tile, and action the same visual weight.

A useful default hierarchy is:

1. **Primary state / primary measurement / primary content**
2. **Primary action**
3. **Mode or screen identity**
4. **Immediate secondary values**
5. **Progress / phase / status**
6. **Metadata and tertiary context**

The language benefits from **extreme differences in scale**.

A giant number beside tiny technical labels is more characteristic than six medium cards.

Use visual imbalance intentionally.

---

# 7. Every screen should have character

The product must feel coherent without every screen being the same template.

Design each screen according to its job.

Possible composition engines:

- **Home / launcher:** spacious, identity-led, strong mode fields
- **Selection / inventory:** strict modular grid or stacked rows, decisive selected state
- **Live / running:** measurement matrix, strong primary readout, calm recent history, persistent action
- **Results:** analytical / report-like / receipt-like, dense but ordered
- **Fault / interruption:** abrupt, simplified, confrontational
- **Setup:** structured, instructional, progressive disclosure
- **Detail / diagnostics:** data-dense, tabular, compact, highly systematic

Family resemblance comes from tokens, typography, symbols, button language, borders, and spacing — not from forcing every screen into one arrangement.

---

# 8. Color is architecture

## 8.1 Neutral field

Begin with a very dark neutral system:

- near-black background
- slightly raised charcoal/navy surfaces
- restrained structural gray
- off-white primary text
- muted cool secondary text

Pure black is acceptable when technically useful, but a very dark neutral often gives more room for layering.

## 8.2 Saturated colors occupy real area

Mode/status colors are not tiny accents.

Use them as:

- full button fills
- selected rows
- headers
- large state slabs
- active tabs
- icon tiles
- progress blocks
- whole-screen interruption regions

> **Color should participate in composition, not merely decorate it.**

## 8.3 Black knockout text on bright color

A defining pattern is:

> **saturated solid rectangle + black text / black icon**

This creates a print, signage, sports-graphics, and industrial-wayfinding feeling rather than a glowing sci-fi HUD feeling.

Use it for high-confidence actions, active selections, prominent navigation, and mode identity.

## 8.4 Stable semantic palette

When the new product has meaningful domains or modes, give each one a stable identity color.

A strong family might include:

- acid lime / chartreuse
- hot magenta
- electric cyan
- signal orange
- violet or intense blue where needed
- red reserved for danger / destructive actions

Do not copy this palette mechanically if the new product needs different semantics. Preserve the principle: **major functional domain = memorable, stable color identity.**

## 8.5 Reserve danger colors

Red should stay scarce enough to mean something.

Use it for:

- Stop
- destructive actions
- faults
- hard blocks
- safety-critical interruption

Do not consume red for normal navigation simply because the screen needs another accent.

---

# 9. Typography: editorial, technical, aggressive

The type system should feel closer to industrial labeling, technical publishing, sports graphics, wayfinding, or engineered instrumentation than fictional spaceship UI.

## 9.1 Type characteristics

Prefer families with:

- clear grotesk / neo-grotesk construction
- strong uppercase forms
- excellent numerals
- compact or moderately condensed proportions
- good small-size rendering
- useful weight range

For numeric/diagnostic contexts, a compatible mono or tabular-numeral treatment can be valuable.

## 9.2 Extreme scale contrast

Use a much larger gap between the biggest and smallest type than conventional app UI.

Typical roles:

- enormous measurement / count / status
- bold screen or mode title
- blunt action text
- compact uppercase field labels
- very small technical metadata

The largest text can become part of the composition rather than simply a readable label.

## 9.3 Microtype is texture only when it is real

Useful microcopy includes:

- state IDs
- timestamps
- units
- channels
- mode labels
- revision/version labels
- actual system identifiers
- actual category/status names

Do not create personality with fake hexadecimal strings, fake serial numbers, fake telemetry, or invented lore.

Meaningful metadata is more convincing than decorative nonsense.

## 9.4 Numeric discipline

For measurements and live data:

- use stable decimal precision
- align related numbers consistently
- prefer tabular numerals when available
- attach units clearly
- reserve precision appropriate to the sensor/data source
- never imply more accuracy than the underlying data supports

---

# 10. Symbol / icon language

The key lesson from the previous project is that icons must be **a system, not a collection of illustrations**.

## 10.1 Choose one construction grammar

Define:

- one grid family
- one module thickness
- one bounding-box logic
- one corner logic
- one visual weight
- one negative-space philosophy
- one scaling rule

Then construct every core symbol from it.

A symbol family can use:

- 3×3, 5×5, 7×7, or another deliberately chosen modular grid
- monoline geometry
- square cell occupancy
- block-built marks
- aperture / fiducial logic

The exact grid should suit the new display. **Consistency matters more than a particular cell count.**

## 10.2 Symbols should have character without noise

The desired feeling is something between:

- machine-vision fiducial
- aerospace registration mark
- pixel glyph
- industrial pictogram
- compact technical insignia

But the symbol should remain immediately recognizable in context.

Avoid random QR-like noise.

## 10.3 Same symbol everywhere

Do not redraw a similar icon separately for Home, header, tab, and results.

Use one canonical definition and render/scale from that source.

For implementation, prefer deterministic vector/path/primitive definitions where practical rather than screenshots of symbols.

## 10.4 Safety does not require cryptography

Critical warning states should use explicit language and obvious shape/state treatment. Do not force the operator to learn an abstract glyph before understanding a dangerous condition.

---

# 11. Buttons and interaction surfaces

Buttons are one of the strongest carriers of the language.

## 11.1 Primary action

Preferred pattern:

- full solid saturated fill
- black knockout text
- bold label
- optional canonical symbol cell
- strong rectangular geometry
- little or no corner radius

## 11.2 Secondary action

Preferred pattern:

- dark field
- restrained border or structural separation
- off-white text
- lower visual weight than primary

## 11.3 Destructive / emergency action

Preferred pattern:

- solid red field
- black or very high-contrast text/symbol
- simple label
- visually isolated enough to be unmistakable

## 11.4 Selected state

Do not rely only on a tiny outline change.

Strong selected states can use:

- full-color fill
- inverted text
- filled symbol tile
- strong border thickness shift
- architectural movement of the block

Selection should be obvious at peripheral glance.

## 11.5 Details can live inside the control

A large interactive row can contain:

- symbol cell
- main label
- compact subtitle
- trailing state/value
- tiny index/metadata

But all of it should remain one coherent hit target where appropriate.

The interaction target should be simpler than its graphic detailing suggests.

---

# 12. Technical ornament: disciplined density

The contemporary language supports dense micro-detail, but it must remain subordinate to function.

Useful peripheral devices:

- checker blocks
- registration marks
- tiny square pips
- barcode-like dividers
- short tick rails
- corner locators
- state strips
- small brackets
- index labels
- compact status matrices
- baseline/grid fragments

Rules:

1. Ornament belongs mainly at edges, separators, headers, and metadata regions.
2. Never reduce readability of primary controls or measurements.
3. Repetition should feel systematic.
4. It should look plausible as part of a real information system.
5. A screen should still work if most ornament is removed.

Think **technical publishing**, not “add random cyberpunk glyphs.”

---

# 13. Data visualization

Charts and indicators should feel integrated into the editorial system rather than dropped in from a generic dashboard library.

Prefer:

- hard rectangular chart areas
- restrained grids
- direct labeling
- mode-colored process lines
- simple dashed/reference lines
- compact legends
- block progress tracks
- stepped stage indicators
- dense small-multiple presentation where appropriate

Avoid:

- glowing neon charts
- unnecessary gradients
- 3D graphs
- ornamental radar displays
- smoothing that misrepresents the data

A chart is still a measurement instrument.

If data is missing, stale, synthetic, estimated, or outside scope, communicate that truthfully rather than drawing a decorative continuous trace.

---

# 14. Motion

The source language can support expressive motion, but motion should reflect context.

Possible expressive transitions:

- block wipes
- hard field replacements
- sliding registration elements
- brief text corruption / reconstruction
- clipped number transitions
- staggered modular reveals

Use high-energy motion mostly for:

- navigation
- mode change
- boot / entry
- achievement / completion
- non-critical scene transitions

Active monitoring, critical controls, and faults should become calmer and more deterministic.

Do not invent exact timing from a trailer. Prototype motion for the target hardware/software stack and measure it.

---

# 15. Density principle: dense metadata, sparse decisions

A screen may look information-rich while remaining easy to operate.

The trick is to keep **decision density low**.

Good:

- one obvious primary action
- one obvious state
- one dominant value
- lots of compact supporting context

Bad:

- seven equally prominent buttons
- every field inside its own card
- every status rendered at the same weight
- decorative metadata competing with real controls

> **The interface can be visually dense without being cognitively dense.**

---

# 16. Whitespace is part of the style

Do not fill every empty region.

Large areas of dark negative space are useful for:

- increasing contrast
- creating tension
- making saturated blocks more forceful
- emphasizing giant typography
- separating primary action from metadata

Asymmetry should often come from **scale and whitespace**, not from angled geometry.

---

# 17. Screen-state design approach

Do not design only the attractive “normal” screen.

Before finalizing the system, inventory all meaningful states for the new product:

- empty / initial
- loading / connecting
- normal idle
- selected
- active / running
- paused if the product truly supports it
- partial / incomplete
- success / complete
- unavailable / disabled
- blocked with reason
- degraded
- warning
- fault
- destructive confirmation if needed
- no data
- stale data
- permissions / authentication if applicable
- setup / calibration
- results / history / diagnostics

Design them from the same visual grammar.

Never invent state semantics merely to fill a concept board.

---

# 18. Truthfulness is part of the aesthetic

One of the most important process rules from the previous project:

> **A visually convincing screen must never imply data, capability, validation, sensing, or state that the real system does not have.**

Examples of what to avoid:

- invented sensor readings
- fabricated history
- “stable” without a defined stability condition
- “safe” without a real safety determination
- “validated” without performed validation
- fake runtime values shown as real
- a button that has no implementation path
- a control that silently changes system semantics

During visual exploration, synthetic data is fine when clearly identified as synthetic fixture data.

Final implementation media should be generated from the actual UI code wherever practical.

---

# 19. Research → design → implementation workflow

Use this sequence on the new project.

## Phase A — understand the product

Before styling anything, determine:

- display size, aspect ratio, pixel density, color capability
- input mechanism: touch, rotary, buttons, pointer, keyboard, etc.
- viewing distance and environmental constraints
- software/UI stack
- typography/font constraints
- memory and rendering constraints
- available sensors/data
- product modes and state machine
- actions and destructive actions
- safety/business-critical behavior
- update rates and latency expectations
- localization/accessibility needs

Do not carry old hardware assumptions into the new product.

## Phase B — build a state/action inventory

Create a matrix of:

- screen/state
- information shown
- primary action
- secondary action
- exit path
- disabled/blocked condition
- data source
- update cadence
- implementation owner

This is the behavioral skeleton.

## Phase C — research the visual corpus

Gather actual contemporary Marathon UI and first-hand expressive graphics. Annotate what each reference proves.

Do not begin by generating generic “Marathon-like” posters from memory.

## Phase D — define tokens and grammar

Establish:

- neutrals
- semantic/mode colors
- typography roles
- spacing rhythm
- border/radius rules
- icon construction grammar
- button hierarchy
- chart treatment
- metadata treatment
- motion principles

## Phase E — explore composition engines

Design several fundamentally different screen compositions within the same language.

Aim for variation in:

- dominant scale
- field placement
- direction of reading
- color-field architecture
- negative space
- data density

Do not merely recolor one template.

## Phase F — stress-test the language

Apply it to:

- the quietest screen
- the densest screen
- the most urgent screen
- the most analytical screen
- a no-data or blocked state

If the design language only works on Home, it is not a design system.

## Phase G — implement source-of-truth components

Once the art direction is accepted, encode:

- color tokens
- typography tokens
- spacing tokens
- canonical symbols
- reusable controls
- common structural components

The implementation, not the concept image, should become authoritative.

## Phase H — render from code

Where the stack permits it, create deterministic screenshots from the actual UI implementation at the actual target resolution.

Review those renders at:

- 1× native size
- enlarged pixel inspection
- expected physical viewing size if possible

Do not continue refining mockups while the actual implementation drifts elsewhere.

---

# 20. Implementation discipline

The previous project established several process rules worth carrying to any stack.

## 20.1 Separate appearance from behavior

Visual redesign should not accidentally rewrite application logic.

Create clear boundaries between:

- product/state logic
- commands/actions
- view state
- theme/tokens
- rendering/components
- assets

If a screen suggests a new capability, treat it as a product/behavior proposal, not a harmless visual change.

## 20.2 Keep canonical tokens in code

Colors, symbol definitions, spacing, and core dimensions should live in one source of truth where possible.

Generate review assets from those definitions rather than manually recreating them.

## 20.3 Prefer deterministic assets

For geometric identity marks and icons, prefer:

- vector paths
- integer geometry
- primitive drawing
- reusable component definitions

over raster images that have been independently redrawn.

## 20.4 Preserve reversibility

For integration into an existing codebase:

- make focused changes
- document touched files
- preserve unrelated behavior
- use diff/patch/branch workflows
- provide rollback or easy reversion
- detect conflicts rather than force overwriting newer work

## 20.5 Tests should reflect interaction truth

Test meaningful behavior, not only screenshot similarity.

Depending on the new product, cover:

- hit targets
- click/tap/hold behavior
- disabled states
- destructive-action priority
- state preemption
- navigation back paths
- stale queued actions
- resizing/overflow
- missing data
- extreme values
- localization stress
- resource stability

Visual tests and behavioral tests complement each other.

---

# 21. Implementation handoff standard

A good design handoff should contain more than screenshots.

Recommended deliverables:

1. **Design-language document** — this doctrine plus project-specific adaptations.
2. **Reference pack** — clearly classified sources and observations.
3. **Tokens** — palette, type scale, spacing, radii/borders, symbol rules.
4. **Screen/state matrix** — complete functional coverage.
5. **Native-resolution mockups or renders** — preferably from code once implementation begins.
6. **Component definitions** — button, row, header, status, chart, modal/fault, etc.
7. **Canonical symbol assets** — one source per symbol.
8. **Interaction rules** — selection, press, hold, disabled, fault, loading.
9. **Implementation notes** — stack-specific mapping and constraints.
10. **Verification record** — what was actually tested and what remains unverified.

Do not call a concept board a final implementation specification if it contains invented states or values.

---

# 22. What NOT to carry into the new project

The following belonged to the previous product and must **not** be treated as part of the Marathon visual language:

- 240×320 screen size
- portrait orientation
- LVGL
- ESP32
- TFT_eSPI
- specific Montserrat pixel sizes
- toaster-oven modes
- reflow / anneal / chamber / heater-test semantics
- lime = reflow, magenta = anneal, etc.
- PID tuning
- thermocouple/probe rules
- Stop placement from the old display
- old temperature values
- old margins/button sizes
- previous safety state machine
- A/B/C or fixed-probe commissioning workflows

The **methods and visual grammar** transfer. The old product's literal tokens and behavior do not.

---

# 23. Common failure modes / anti-patterns

Reject the design when it drifts toward any of these:

### Generic spaceship HUD
Too many thin glowing lines, circular reticles, translucent panes, cyan-on-black everywhere.

### Diagonal-shard overload
Large angled panels and triangular cuts become the entire identity.

### Random QR noise
Pixel/fiducial details have no consistent construction grammar.

### Rounded SaaS cards
Everything becomes equally padded, softly rounded, and visually polite.

### Neon accent syndrome
Saturated colors appear only as tiny strokes and never structure the layout.

### Equal-weight dashboard
Every metric lives in the same box and no information dominates.

### Fake technicality
Hex strings, IDs, pseudo-telemetry, and lore exist only as decoration.

### Poster before product
A gorgeous concept shows controls/states the software does not actually support.

### Template repetition
Every screen uses exactly the same header/cards/footer layout with different labels.

### Style swallowing safety/usability
A destructive action becomes obscure, a live value becomes unreadable, or an urgent state becomes visually ambiguous for the sake of visual novelty.

---

# 24. Expressiveness dial

The design can be tuned without changing its identity.

## Level 1 — restrained instrument

- mostly dark neutral
- small mode accents
- few ornamental marks
- strict conventional hierarchy

## Level 2 — strong product identity

- full-color selected actions
- clear icon tiles
- dramatic headline/value scale
- modest technical ornament

## Level 3 — target default

- large saturated structural fields
- aggressive black knockout controls
- strong asymmetry
- giant/small typography contrast
- meaningful metadata rails
- screen-specific compositions

## Level 4 — expressive / promotional edge

- oversized graphic crops
- bolder field replacement
- more registration/checker language
- unusual typographic composition
- expressive transitions

For real operational screens, **Level 3** is usually the sweet spot. Level 4 is best used selectively for entry, transitions, identity moments, or non-critical contexts.

---

# 25. New-project kickoff checklist

Before making the first mockup, the next session should answer:

- What is the target display resolution/aspect ratio?
- What is the physical size and viewing distance?
- What are the input methods?
- What software/rendering stack is available?
- Which fonts can actually be used?
- What are the major modes/domains?
- Which state or value deserves the largest visual weight?
- What actions are primary, destructive, reversible, or safety-critical?
- What real metadata exists?
- What data can be missing or stale?
- Which colors need established semantics?
- What states must be designed beyond the happy path?
- What are the rendering/memory/performance constraints?
- Can actual UI code be rendered deterministically for review?
- Which Marathon references are gameplay UI versus promotional expression?

Only then should project-specific tokens be chosen.

---

# 26. Ready-to-paste design-session seed

Use the following when starting a new session:

> We are designing an original interface using a contemporary Marathon-inspired visual language. Do not copy Marathon branding, logos, lore, exact layouts, or proprietary assets. Extract the graphic grammar instead.
>
> Treat the interface as industrial editorial graphic design that happens to be interactive. Use primarily orthogonal modular composition: hard rectangles, grids, rails, apertures, full-width slabs, stacked modules, bold crops, and intentional negative space. Avoid generic sci-fi HUD tropes, glow, glassmorphism, excessive rounded cards, random QR noise, and oversized diagonal shards.
>
> Use saturated color as structural material, not tiny decoration. Important selected states and primary actions can become full-color fields with black knockout text/icons. Give major functional domains stable colors where appropriate; reserve danger color for genuine danger/destructive actions.
>
> Use extreme typographic hierarchy: huge primary values/states alongside compact technical labels and meaningful metadata. Prefer engineered grotesk typography and excellent numerals over faux-futuristic type. Do not invent telemetry or fake IDs just to create texture.
>
> Build one coherent modular symbol system. All core icons must share the same grid, module thickness, visual weight, bounding logic, and scaling rules. Reuse canonical definitions everywhere instead of independently redrawing similar symbols.
>
> Every screen should have its own composition appropriate to its purpose while remaining clearly part of one product family. Dense metadata is welcome; dense decision-making is not. The primary state and action must remain obvious.
>
> Research actual contemporary Marathon playable UI separately from designer/process work and promotional graphics. Label source types. Convert observations into portable design rules rather than tracing screens.
>
> Do not invent product capabilities, sensor values, successful states, validation, or history. Synthetic concept data must be marked as synthetic. Once implementation begins, treat the real UI code and canonical tokens/assets as source of truth and render native screenshots from it for review.
>
> First understand this new product's display, input, state machine, software stack, data sources, critical actions, accessibility and performance constraints. Do not inherit dimensions or behavior from a previous project. Then establish the screen/state matrix and project-specific tokens before producing final screens.

---

# 27. Approval checklist

A design pass is ready to move forward when the answer to these is “yes”:

### Visual language
- Is the composition predominantly orthogonal and modular?
- Does color occupy meaningful structural area?
- Are black-on-color controls used deliberately where appropriate?
- Is there strong contrast between dominant and supporting typography?
- Are symbols clearly members of one construction family?
- Does the screen feel designed rather than decorated?
- Is technical ornament systematic and subordinate?
- Is negative space used intentionally?
- Do different screens have distinct compositions without losing family resemblance?

### Interaction
- Is the primary action obvious?
- Is selection obvious without relying on tiny cues?
- Are destructive/critical actions unmistakable?
- Are touch/pointer/button targets appropriate to the target hardware?
- Are blocked, loading, empty, failure, and no-data states designed?

### Truthfulness
- Does every displayed measurement/state have a real source?
- Are synthetic fixtures clearly separated from real captured data?
- Does the interface avoid claims the system cannot establish?
- Are units and precision defensible?
- Are status words such as PASS, SAFE, STABLE, COMPLETE, or VALIDATED defined before being shown?

### Implementation
- Are core tokens centralized?
- Are canonical icons reused rather than recreated?
- Can the design actually render on the target stack?
- Has it been inspected at native resolution / realistic viewing size?
- Are visual and behavioral tests planned?
- Is handoff explicit about verified versus unverified behavior?

---

# 28. Final principle

When uncertain, do **less generic UI and more deliberate graphic composition** — but never at the cost of operational clarity.

The most successful interpretation is not “make it look sci-fi.”

It is:

> **Make the information system itself visually expressive.**

That is the transferable core of the Marathon-inspired language established in the previous session.
