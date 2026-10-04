# LOTUS design system

## Shell surfaces

There are four shell planes. Do not introduce a fifth page, panel or chrome
background token.

  | Role                    | Utility             | Token               | light     | dark      | Radius               | Separation                       |
  | ---                     | ---                 | ---                 | ---       | ---       | ---                  | ---                              |
  | Page (canvas)           | `bg-shell-page`     | `--shell-page-bg`   | `#f2f5f8` | `#0a0f19` | none                 | none                             |
  | Header/footer/metadata  | `bg-shell-chrome`   | `--shell-chrome-bg` | `#e7ecf1` | `#1c2637` | header/footer only   | `border-shell-border`, no shadow |
  | Card / raised panel     | `bg-shell-raised`   | `--shell-raised-bg` | `#ffffff` | `#131b29` | `rounded-xl` (12 px) | `border-shell-border`, no shadow |
  | Nested panel            | `bg-panel-soft`     | `--panel-bg-soft`   | `#eef1f5` | `#18212f` | `rounded-xl` (12 px) | `border border-border`           |

The canvas is 1.094:1 from pure white with a +6 blue cast, matching Apple's
grouped background. It is deliberately *not* pure white: white would collapse
page and card to 1.000:1 and erase the plane.

### Separation is a property of adjacent planes, not of every pair

The rule that matters is that **planes a user sees side by side stay
distinguishable**. Measured ratios, element plane against the plane directly
behind it, on `/`, `/search` and `/curation`:

  | Adjacent pair    | light | dark  |
  | ---------------- | ----- | ----- |
  | chrome over page | 1.086 | 1.262 |
  | card over page   | 1.094 | 1.111 |
  | card over chrome | 1.189 | 1.136 |
  | panel over card  | 1.133 | 1.067 |
  | card over panel  | 1.133 | 1.067 |

Every adjacent pair clears 1.08 except the **dark panel on card at 1.067**. That
boundary is carried by the panel's own outline: `--border` on `--panel-bg-soft`
is 4.37:1, far past the 1.4.11 requirement, so the plane reads as a distinct
surface from its border rather than from its fill. Closing the 0.013 gap would
mean re-tuning the `--footer-wd-*` mixes, which are calibrated against both
chrome and the stat cards; that has bitten twice, so the deliberate choice is to
leave the tokens alone and keep the measured value written down here.

Non-adjacent pairs are not constrained --- page and panel are always separated
by a card, so their 1.035:1 (light) is never visible as a direct edge.

### Borders

`--border` outlines interactive controls, so it clears 1.4.11 (3:1) on every
plane it can land on:

  | Plane  | `--border`  | `--border-soft` |
  | ------ | ----------- | --------------- |
  | page   | 3.47 / 5.18 | 1.44 / 2.02     |
  | chrome | 3.20 / 4.11 | 1.33 / 1.60     |
  | card   | 3.80 / 4.66 | 1.58 / 1.82     |
  | panel  | 3.36 / 4.37 | 1.39 / 1.71     |

(light / dark.) `--border-soft` is for decorative hairlines only and is
deliberately below 3:1 everywhere.

### Footer tints

The footer organism badges sit on a `bg-current/14` tint of their own colour, so
`--footer-wd-*` must be tuned against *both* chrome and the stat card. Measured
on the live results view, light and dark: 6.03--10.01:1 and 6.42--6.94:1 against
a 4.5:1 requirement. **Any change to a plane colour invalidates these and
requires re-measuring** --- a `color-mix()` tint cannot be checked by reading
`background-color`, which resolves to `oklab(...)`.

Controls keep their existing 8 px radius and may use the existing subtle shadow.
Shell roles never combine a border and a shadow. A shell has no background when
it is an unframed layout wrapper or an inner region of a raised panel; do not
stack one raised surface inside another merely to create spacing. Tone-colored
notice backgrounds are semantic status surfaces, not shell levels.

Tokens are defined only in `tailwind/styles.css`. Tailwind classes remain inline
in RSX; do not wrap styling rules in Rust components.

## Text-only controls and native radios: measured, and left alone

A control with no visible boundary is often flagged for 1.4.11 on the assumption
that it needs one. Measured on the live results view, in both themes:

  | Control                        | light   | dark    | Required |
  | ------------------------------ | ------- | ------- | -------- |
  | results sort button, label     | 17.74:1 | 15.59:1 | 4.5:1    |
  | results sort button, `⇅` glyph | 7.06:1  | 9.86:1  | 3:1      |
  | native radio, checked          | 6.04:1  | 7.90:1  | 3:1      |
  | native radio, unchecked        | 4.42:1  | 4.62:1  | 3:1      |

The sort buttons and the radios all clear their thresholds, so **no border was
added to any of them**. A border on all six sort buttons would put a 3:1 rule
across every table header to fix a failure that does not exist, and the radio
ring is chosen by the user agent rather than by us --- its contrast is not a
token we control, and `accent-accent` only tints the selected state. Their
affordance is the label plus the `⇅` glyph, the `aria-sort` state, and
hover/focus, all of which are present and all of which are legible at the ratios
above.

The radios were checked by scanning painted pixels from a 1:1 screenshot,
because a user-agent border is invisible to `getComputedStyle` and to any
`rgba()` compositing. Do not "fix" them by styling the ring unless a measurement
says the ring is the problem.

## Semantics and RDFa

Use native HTML first. Every page has one `h1`, followed by ordered `h2`/`h3`
headings; `header`, `nav`, `main`, `section`, `footer`, `table`, `time`, labels,
and native controls are preferred over ARIA replacements. Visible text is the
accessible name; use `aria-label` only for icon-only controls. Stable automation
hooks are `data-lotus-id` for domain entities and `data-segmented-value` for
segmented controls.

## Agent-facing forms: two different attribute families

A `<form>` that agents can drive carries **two unrelated sets of attributes**,
and conflating them is the trap:

  | Family                | Attributes                                                                                          | Read by                  | Purpose                                                                                                                          |
  | ---                   | ---                                                                                                 | ---                      | ---                                                                                                                              |
  | WebMCP (the standard) | `toolname`, `tooldescription`, `toolautosubmit` on the form; `toolparamdescription` on each control | a WebMCP-capable browser | actually registers the form as a tool; the browser synthesises the input schema from the controls' `name`, `type` and `required` |
  | in-house hooks        | `data-webmcp-*` and `data-mcp-*`                                                                    | nothing in the browser   | a parallel, explicit description for tooling that reads them directly                                                            |

The `data-*` hooks predate WebMCP and register **nothing** on their own. A form
with only those attributes is invisible to WebMCP. Keep both, but a new form
needs the `tool*` attributes to be discoverable, and its `data-webmcp-schema`
should stay consistent with the real control names --- that schema is what a
non-browser consumer reads, and when the two disagree the agent sends a property
no field exists for. (`SEARCH_SCHEMA` declared `formula` while the control is
`formula_exact`, so a consumer using it had its filter silently dropped.)

`toolautosubmit` is the difference between "an agent can fill this in" and "an
agent can submit this without you". It is on the search form, which only reads.
It is deliberately absent from both curation forms, which mutate the curation
queue.

Controls whose names are generated at runtime (the per-element formula
min/maximums) get no `toolparamdescription`; the browser still synthesises the
property, it just carries no description.

### And a third registration, imperative, because of when the first one happens

`assets/js/webmcp.js` registers the same capabilities with
`document.modelContext.registerTool`, from a deferred script in the document
head. That is not a third description of the same thing, it is the only one
that exists at document start.

The declarative attributes are rendered by the WASM client, so they appear
seconds after the document has loaded. Anything that inspects the tool surface
then — the Lighthouse `webmcp-registered-tools` and `webmcp-form-coverage`
audits, an agent that opened the page and looked immediately — sees an empty
tool surface and reports the app as having no WebMCP at all. Measured against the
live host with Lighthouse 13.5.0: `webmcp-registered-tools` returned an empty
list and the other two `webmcp-*` audits were not applicable.

Rules for that script:

- **Drive the real controls.** It sets values through the native prototype
  setter and then dispatches `input`/`change`, because Dioxus keeps the criteria
  in component state and the re-render is driven by the event, not by the DOM
  mutation. A tool that called the query API directly would be a second
  implementation of the search, and the user would watch nothing happen.
- **Open the `<details>` first.** The advanced filters are in closed
  disclosures, so a field the agent was told to set does not exist until they
  are open.
- **Register per route.** The curation tools are registered only on
  `/curation`. A tool whose form the agent cannot reach is context it pays
  tokens for and then fails to use.
- **Flag the output.** What the tools return comes from Wikidata and
  third-party services, so it is `untrustedContentHint`.
- **No-op where WebMCP is absent**, and treat a registration refused by
  permissions policy as ordinary rather than as an error.

`the_webmcp_tool_surface_is_loaded_and_names_real_elements` in `build/tests.rs`
is what keeps it honest: it fails if the document stops loading the script, and
if any `#id` the script targets stops being rendered by a component — which is
how a renamed form id would otherwise leave a tool that fills nothing and still
reports success.

Declare `vocab="https://schema.org/"` at the domain-content scope. Use canonical
Wikidata resources for QIDs and established schema.org terms only:

  | Entity/data         | RDFa                                                                          |
  | ---                 | ---                                                                           |
  | Result collection   | `typeof="ItemList"`, `property="numberOfItems"` with `content`                |
  | Compound row        | `typeof="ChemicalEntity"`, `about="https://www.wikidata.org/entity/{QID}"`    |
  | Compound attributes | `schema:name`; Wikidata P233 SMILES, P235 InChIKey, P2062 mass, P274 formula  |
  | Taxon               | Wikidata P171, `typeof="Taxon"`, Wikidata entity `resource`                   |
  | Source article      | Wikidata P248, `typeof="ScholarlyArticle"`, Wikidata entity `resource`        |
  | DOI                 | `property="identifier"`, `resource="https://doi.org/{DOI}"`                   |
  | Publication year    | `time[property="datePublished"]` with `datetime`                              |

Do not annotate decorative controls, labels, or layout wrappers. Do not invent
vocabulary terms. RDFa custom attributes in Dioxus RSX must use quoted HTML
names, for example `"property": "name"`.
