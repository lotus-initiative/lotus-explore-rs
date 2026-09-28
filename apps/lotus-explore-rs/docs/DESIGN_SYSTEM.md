# LOTUS design system

## Shell surfaces

There are four shell planes. Do not introduce a fifth page, panel or chrome
background token.

  | Role                    | Utility             | Token               | light    | dark     | Radius               | Separation                       |
  | ---                     | ---                 | ---                 | ---      | ---      | ---                  | ---                              |
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

  | Adjacent pair              | light  | dark   |
  | -------------------------- | ------ | ------ |
  | chrome over page           | 1.086  | 1.262  |
  | card over page             | 1.094  | 1.111  |
  | card over chrome           | 1.189  | 1.136  |
  | panel over card            | 1.133  | 1.067  |
  | card over panel            | 1.133  | 1.067  |

Every adjacent pair clears 1.08 except the **dark panel on card at 1.067**. That
boundary is carried by the panel's own outline: `--border` on `--panel-bg-soft`
is 4.37:1, far past the 1.4.11 requirement, so the plane reads as a distinct
surface from its border rather than from its fill. Closing the 0.013 gap would
mean re-tuning the `--footer-wd-*` mixes, which are calibrated against both
chrome and the stat cards; that has bitten twice, so the deliberate choice is to
leave the tokens alone and keep the measured value written down here.

Non-adjacent pairs are not constrained — page and panel are always separated by
a card, so their 1.035:1 (light) is never visible as a direct edge.

### Borders

`--border` outlines interactive controls, so it clears 1.4.11 (3:1) on every
plane it can land on:

  | Plane   | `--border` | `--border-soft` |
  | ------- | ---------- | --------------- |
  | page    | 3.47 / 5.18 | 1.44 / 2.02     |
  | chrome  | 3.20 / 4.11 | 1.33 / 1.60     |
  | card    | 3.80 / 4.66 | 1.58 / 1.82     |
  | panel   | 3.36 / 4.37 | 1.39 / 1.71     |

(light / dark.) `--border-soft` is for decorative hairlines only and is
deliberately below 3:1 everywhere.

### Footer tints

The footer organism badges sit on a `bg-current/14` tint of their own colour, so
`--footer-wd-*` must be tuned against *both* chrome and the stat card. Measured
on the live results view, light and dark: 6.03–10.01:1 and 6.42–6.94:1 against a
4.5:1 requirement. **Any change to a plane colour invalidates these and requires
re-measuring** — a `color-mix()` tint cannot be checked by reading
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

| Control                          | light   | dark    | Required |
| -------------------------------- | ------- | ------- | -------- |
| results sort button, label       | 17.74:1 | 15.59:1 | 4.5:1    |
| results sort button, `⇅` glyph   | 7.06:1  | 9.86:1  | 3:1      |
| native radio, checked            | 6.04:1  | 7.90:1  | 3:1      |
| native radio, unchecked          | 4.42:1  | 4.62:1  | 3:1      |

The sort buttons and the radios all clear their thresholds, so **no border was
added to any of them**. A border on all six sort buttons would put a 3:1 rule
across every table header to fix a failure that does not exist, and the radio
ring is chosen by the user agent rather than by us — its contrast is not a token
we control, and `accent-accent` only tints the selected state. Their affordance
is the label plus the `⇅` glyph, the `aria-sort` state, and hover/focus, all of
which are present and all of which are legible at the ratios above.

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
segmented controls; existing `data-mcp-*` hooks remain the schema-oriented form
API.

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
