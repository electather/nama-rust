---
name: Nama
description: A self-hosted media server with Requests built in. Quiet chrome, loud artwork.
colors:
  background: "#0e0e0e"
  well: "#090909"
  card: "#191919"
  popover: "#222222"
  muted: "#303030"
  secondary: "#a3a3a326"
  foreground: "#ededed"
  foreground-soft: "#c3c3c3"
  muted-foreground: "#8f8f8f"
  border: "#a3a3a326"
  input: "#6e6e6e"
  primary: "#f5f5f7"
  primary-foreground: "#000000"
  primary-dim: "#a1a1a6"
  primary-tint: "#3a3a3c"
  link: "#8fd6b0"
  destructive: "#ff6467"
  destructive-foreground: "#0e0e0e"
  warning: "#e8b93a"
  scrim: "#000000b3"
  track: "#ffffff40"
typography:
  display:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "40px"
    fontWeight: 500
    lineHeight: "42px"
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "28px"
    fontWeight: 500
    lineHeight: "34px"
    letterSpacing: "-0.02em"
  title:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "18px"
    fontWeight: 600
    lineHeight: "24px"
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: "28px"
  section:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "16px"
    fontWeight: 500
    lineHeight: "22px"
  ui:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "20px"
  ui-strong:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: "20px"
  caption:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "16px"
rounded:
  xs: "4px"
  sm: "6px"
  md: "10px"
  lg: "16px"
  xl: "22px"
  full: "9999px"
spacing:
  space-2xs: "2px"
  space-xs: "4px"
  space-sm: "8px"
  space-md: "10px"
  space-lg: "14px"
  space-xl: "20px"
  space-2xl: "30px"
  space-3xl: "48px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-foreground}"
    typography: "{typography.ui}"
    rounded: "{rounded.full}"
    padding: "0 10px"
    height: "32px"
  button-outline:
    textColor: "{colors.foreground}"
    typography: "{typography.ui}"
    rounded: "{rounded.full}"
    padding: "0 10px"
    height: "32px"
  button-ghost:
    textColor: "{colors.foreground-soft}"
    typography: "{typography.ui}"
    rounded: "{rounded.full}"
    padding: "0 10px"
    height: "32px"
  button-destructive:
    textColor: "{colors.destructive}"
    typography: "{typography.ui}"
    rounded: "{rounded.full}"
    padding: "0 10px"
    height: "32px"
  input:
    textColor: "{colors.foreground}"
    typography: "{typography.ui}"
    rounded: "{rounded.lg}"
    padding: "0 10px"
    height: "32px"
  slider:
    backgroundColor: "{colors.well}"
    rounded: "{rounded.md}"
    padding: "2px"
    height: "32px"
  menu:
    backgroundColor: "{colors.popover}"
    textColor: "{colors.foreground-soft}"
    rounded: "{rounded.xl}"
    padding: "4px"
  menu-item:
    textColor: "{colors.foreground-soft}"
    typography: "{typography.ui}"
    rounded: "{rounded.sm}"
    padding: "0 10px"
    height: "40px"
  dialog:
    backgroundColor: "{colors.popover}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.xl}"
    padding: "24px"
  card:
    backgroundColor: "{colors.card}"
    rounded: "{rounded.lg}"
    padding: "20px"
---

# Design System: Nama

## Overview

**Creative North Star: "The Dimmed Living Room"**

Nama should feel like a living room with the lights turned down before the film starts. The interface is the room: the page is near black, the text is soft, and the posters, stills and backdrops carry almost all the colour. Like the Apple TV app, Nama puts artwork first, leaves generous space and keeps its chrome so quiet you stop noticing it. Controls are built like an instrument panel, recessed into wells with a visible rim, and one near-white lights whatever is selected or playing.

Density is relaxed on member surfaces (browse, title, player, Requests) and gets tighter only inside admin views, which are progressively disclosed. Motion confirms and connects: it gives feedback and makes transitions continuous, but nothing is choreographed for show.

The system rejects Plex's busy home screen with its promoted rows and upsells, the generic Netflix-clone look of black and red with autoplaying hero carousels, the *arr / admin-panel aesthetic of Bootstrap tables and dense forms, and Jellyfin's heavy cards and inconsistent spacing.

**Key Characteristics:**
- Artwork is the loudest element on every screen. The interface never competes with it.
- Pure grey chrome up to a near-white `primary`. Every hue comes from the artwork, `link` and the colours of states.
- Solid white acts, a lit grey marks.
- One clean sans (Geist) in several weights. No second family.
- Flat surfaces separated by tone and a hairline; shadows only for what floats.

## Colors

One theme, dark. The neutrals have no chroma, so nothing in the chrome competes with the artwork. In code these are shadcn's semantic variables in `web/src/styles.css` (ADR 0016), used as `bg-card`, `text-foreground-soft`, `border-input` and so on.

### Primary
- **Near-white** (`primary`): the white of Apple TV's buttons. It has two jobs. *Acting*: a solid `primary` fill with black `primary-foreground` ink is the one primary action on a screen (Play, Resume, or Request for a missing title); it also draws seek handles and the focus `ring`. *Marking*: the **lit state** is a `primary-tint` fill, `primary` (or `foreground`) ink and a 1px `primary-dim` outline, for a selected tab, segment or chip, a toggle that's on, and the played part of a seek track or slider.
- **Primary dim** (`primary-dim`): the knob of a toggle that is on and the 1px outline of a lit state. Never text.
- **Primary tint** (`primary-tint`): the fill of a lit state. It carries `primary` (10.4:1), `foreground` (9.7:1) or `foreground-soft` (6.4:1), never `muted-foreground` (3.5:1). It sits only 1.7:1 off the page, so it always takes its `primary-dim` outline.

### Neutral
- **Page** (`background`): the page. Everything else sits on it.
- **Well** (`well`): the one step down, for recessed troughs: select and toggle bodies, segmented troughs, slider and seek tracks. Always behind an `input` rim.
- **Card** (`card`): panels one step up: poster cards, list rows, the settings pane and nav, filter panels, the cells inside a well, the buffered part of a seek track.
- **Popover** (`popover`): what floats: menus, dialogs, the selected row of a settings nav.
- **Muted** (`muted`): hover fill, disabled fill, the knob of a toggle that is off. `accent`, shadcn's hover background, aliases it and is never `primary`.
- **Secondary** (`secondary`): a translucent grey for chips, the tabs trough, the pill behind the current top bar link and the top bar's tray. Its ink is `foreground-soft`; over scrimmed artwork it is `foreground`.
- **Ink**, three levels: `foreground` for titles and whatever is selected or focused; `foreground-soft` for body copy and control labels; `muted-foreground` for metadata, timecodes, inactive labels and resting icons (4.9:1 or better on `background`, `well`, `card` and `popover`). On `muted`, `secondary` and `primary-tint`, use `foreground-soft`.
- **Hairline** (`border`): decoration: card and poster edges, dividers. Below 3:1 on purpose.
- **Rim** (`input`): the boundary of every well and control, 3.1:1 or better on `background`, `well`, `card` and `popover`. At 30% it is also the fill of a text field and an outline Button. Never swap `border` and `input`.
- **Scrim** (`scrim`): under any text or control over artwork.
- **Track** (`track`): the unfilled part of a poster's Watch State bar, always on the poster's `scrim` shade.

### States and links
- **Destructive** (`destructive`): ink on its own 10% fill (`bg-destructive/10`): the confirm button for something that can't be undone, and Declined and Failed labels. `destructive-foreground` is the ink on a solid `destructive` fill.
- **Warning** (`warning`): ink on `bg-warning/10` for states waiting on someone: Pending for an admin, an Unmatched item.
- **Link** (`link`): a soft green for inline links in running facts: people and Libraries. Underlines on hover and focus.

### Named Rules
**The One Voice Rule.** Solid `primary` belongs to one element per screen, the primary action, and covers under a tenth of it. Anything merely selected, on or current takes the lit state. `link` is the one exception to the rule's no-hue chrome, for inline links in running facts only.

**The Artwork Owns Colour Rule.** Interface chrome never uses saturated colour for decoration. Every hue that isn't `link` or a state comes from the media.

**The Never Colour Alone Rule.** Request states (Pending, Approved, Declined, Processing, Partially Available, Available, Failed) and Match status always pair colour with a label or icon. `destructive` and `warning` are close in lightness, and the lit state differs from a plain cell mostly by its outline.

**The Banned Pairings Rule.** No `input` rim on `muted` (2.6:1), on `secondary` over `card` (2.7:1) or on `primary-tint` (2.2:1): a lit state takes its `primary-dim` outline instead. Text on `secondary` over artwork is `foreground`, never `foreground-soft`.

## Typography

**Display Font:** Geist Variable (with `sans-serif`), bundled through Fontsource
**Body Font:** the same family
**Label/Mono Font:** none

**Character:** one clean sans in weights 400, 500 and 600, plus 800 for the shelf watermark alone. Large titles go medium and tight-tracked, so they feel calm rather than loud. Hierarchy comes from weight and size, not from a second family.

### Hierarchy
- **Display** (`text-display`: 500, 40/42px, -0.035em): a Movie or Show title on its title page, over the backdrop's scrim.
- **Headline** (`text-headline`: 500, 28/34px, -0.02em): page, Library and shelf headings.
- **Title** (`text-title`: 600, 18/24px, -0.01em): the title of a list row or a settings pane.
- **Body** (`text-body`: 400, 16/28px): overviews, descriptions and fact lists, capped at 65–75ch.
- **Section** (`text-section`: 500, 16/22px): a settings group heading, the title on a poster card, a Dialog's title.
- **UI** (`text-sm`: 400, 14/20px): control labels, settings rows, menu and nav items; medium on a Button.
- **UI strong** (`text-sm font-semibold`): whatever is selected.
- **Caption** (`text-xs`: 400, 12/16px): metadata, timecodes, tick and state labels. Sentence case. Numbers that change take tabular figures.

**The shelf watermark** is the huge dim word behind one of the first shelves' headings ("Up Next"). It has no token: `text-foreground-soft/10`, `font-extrabold`, `tracking-[-0.04em]`, `leading-[1.17]` and `text-[clamp(4rem,13cqi,10.5rem)]`, in the one shelf component, `aria-hidden`. It never says anything the heading doesn't.

### Named Rules
**The One Family Rule.** Nama uses one typeface. Hierarchy comes from weight and size, never from adding a display or mono face.

**The Legible Over Art Rule.** Text set over artwork always sits on `scrim`, or on a solid panel, so it keeps 4.5:1 whatever the image.

## Layout

Spacing runs on tens, with small steps for the insides of controls. There are no spacing variables: each step is a Tailwind number on its 4px scale.

| Step | Tailwind | For |
| --- | --- | --- |
| `space-2xs` 2px | `0.5` | Padding inside a well, and between its cells |
| `space-xs` 4px | `1` | Tabs trough and nav item padding; icon to label in a dense control |
| `space-sm` 8px | `2` | Gaps inside a control, between chips and player bar buttons |
| `space-md` 10px | `2.5` | The default gap: settings rows, poster card padding, fact rows; a control's own padding |
| `space-lg` 14px | `3.5` | Horizontal padding of a large control; poster to caption |
| `space-xl` 20px | `5` | Panel and list-row padding; between poster cards |
| `space-2xl` 30px | `7.5` | Between settings groups and between list rows |
| `space-3xl` 48px | `12` | Between shelves and page sections |

Sizes are Tailwind steps too: a control (button, text field, select, toggle, segmented) is `h-8`, a large one `h-9`; a tab is `h-11`, the touch-target floor on member screens; a nav, menu or filter row is `h-10`; the player bar `h-13.5`; a settings row's control column `w-80`; the top bar `h-16`, with `size-9` search and user buttons.

## Elevation & Depth

Flat by default. Surfaces step up by tone (`well` below the page, then `background`, `card`, `popover`) and a `border` hairline, never by a shadow at rest.

### Shadow Vocabulary
- **Edge** (`inset-shadow-edge`: `inset 0 1px 2px rgb(255 255 255 / 0.08)`): a faint top light on a raised fill. Buttons don't carry it.
- **Pop** (`shadow-pop`: `0 4px 14px rgb(0 0 0 / 0.35)`): menus and select lists.
- **Float** (`shadow-float`: `0 8px 15px rgb(0 0 0 / 0.46)`): dialogs, the player bar and overlays over video, the top bar once it lifts.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat at rest. If a poster grid needs shadows to separate its items, the spacing is wrong.

## Shapes

Radii nest: a shape inside another takes the next radius down, so the corners stay parallel.

- `rounded-xs` (4px): a field nested in a cell.
- `rounded-sm` (6px): cells in a well, slider and seek handles, nav and menu items, small badges.
- `rounded-md` (10px): wells, the poster inside a poster card.
- `rounded-lg` (16px): text fields, cards, list rows, bare posters, stills and backdrops, the active tab.
- `rounded-xl` (22px): menus, dialogs, popovers, the tabs trough.
- `rounded-full`: buttons, chips, state labels, progress bars.

No other radius exists: `styles.css` resets Tailwind's.

## Components

### Buttons
- **Shape:** fully rounded, `h-8` with `space-md` padding, `text-sm` medium, a 16px icon. An icon-only button is an `h-8` circle named by its `aria-label`. Play or Resume at the top of a title page is `h-11` with 16px text and a filled ▶.
- **Primary** (`default`): solid `primary` with black ink, once per screen. Hover drops it to four-fifths strength.
- **Outline:** an `input` rim over `input` at 30%, `foreground` ink. Cancel in a dialog, or a second way to do the same thing.
- **Ghost:** `foreground-soft`, no fill until hover, then `muted` at half and `foreground` ink. Toolbars, and beside a primary.
- **Destructive:** `destructive` ink on `bg-destructive/10`, only for the confirm button of a dialog that asks before something can't be undone (revoking a Device, deleting a User).
- **Overlay:** `foreground` ink over video, `secondary` on hover, a full-strength ring on focus.
- **Focus / disabled / press:** focus turns the rim `ring` with a 3px `ring` at half strength, flush with the edge; disabled fades to half strength; a press nudges it down 1px.

### Inputs
An `input` rim over `input` at 30%, `rounded-lg`, `h-8`, `space-md` padding, `foreground` ink and a `muted-foreground` placeholder. Text is 16px on phones (so iOS doesn't zoom) and 14px from `md`. Focus turns the rim `ring` with a 3px `ring` at half strength; `aria-invalid` turns rim and ring `destructive`; disabled fades to half strength. A textarea and a native select take the same recipe. Never use a placeholder as the only label.

### Sliders
A rimmed `well`, `rounded-md`, as tall as the controls beside it (`h-8`), padded `space-2xs`. The set part is a `primary-tint` block ending in an 8px solid `primary` handle, `rounded-sm`, that stays inside the well. Focus draws a 2px `ring` 2px off the well. The player's position and volume are both sliders.

### Progress
The Watch State bar and other progress: 6px, `rounded-full`, a `foreground` fill on `track`. On a poster it sits on the poster's `scrim` shade.

### Menus
`popover`, `rounded-xl`, a `border` hairline and `shadow-pop`, `space-xs` padding, at least 240px wide, opening on the exponential ease-out. Items are `h-10`, `rounded-sm`, `text-sm` in `foreground-soft` with `muted-foreground` icons; a highlighted item turns `muted` with `foreground` ink, and a chosen one is semibold `foreground` with a check. Keep the destructive item last.

### Dialogs
`popover`, `rounded-xl`, a `border` hairline and `shadow-float`, 24px padding, over `scrim`. The title is `text-section`, the description `text-sm` in `muted-foreground`. The footer is an outline Cancel then the one action, right-aligned, stacking under 640px. A native modal `<dialog>`, so the browser traps and restores focus.

### Cards
`card`, `rounded-lg`, a `border` hairline, no shadow.

### Posters and artwork
- **Posters**: 2:3, `rounded-lg`, a `border` hairline. The Watch State bar sits under a poster that has one.
- **Backdrop cards**: 16:9, `rounded-lg`, a `border` hairline, with the title over the art in Headline on the scrim, or its Title Logo, up to 48px tall and 60% of the card wide, and a caption line under the card. A Library's says its kind. A Continue Watching card puts ▶ and the Watch State bar under the title, and under the card which Episode and how much is left, such as "S1, E3 · 33m left" or "1h 12m left". Without a Backdrop, the box is plain `muted` with the title in it.
- **Title page top**: a Movie's or Show's page opens on its Backdrop, full width under the app bar, scrolling away with the page. The art is clear above the title, and the title sits on at least the scrim however many lines it takes, fading into the page at the bottom. The title is its Title Logo, up to 112px tall (144px from `sm`) and 80% wide (448px from `sm`), or else its title in Display, and either way the `<h1>`. Under the art go a caption line of what it is, its genres and its year and runtime (a Show: its first air year), then its Content Rating and, on a Movie, its badges (4K, HDR or Dolby Vision, SDH), each `rounded-sm` with an `input` rim. Then the Play or Resume button, with the Watch State bar and what's left beside Resume, then the overview in Body, clamped to two lines with a ghost "More" that turns to "Less". A Request page shares only the Backdrop and title, always as text.
- **Episode cards**: a Show's Episodes, in the Backdrop cards' grid. The Still is 16:9, `rounded-lg`, a `border` hairline, with nothing over it, and without one the box is plain `muted`. Under it go a caption naming the Episode ("Episode 3", "Specials, Episode 3"), the title in Title, the synopsis as a caption clamped to three lines, a caption line of air date, runtime and the Version's picture and sound, the Watch State bar once started, then the played toggle and Delete.
- **State labels**: `rounded-full`, with an icon and the state's word. Most are neutral: a `border` hairline and `muted-foreground`. Available takes the lit state; Declined and Failed take `destructive` on `bg-destructive/10`; a state waiting on someone takes `warning` on `bg-warning/10`.

### Motion
Hover and press change in 150ms; the player bar fades in 200ms; the top bar lifts, and a poster opens into its title page, in 300ms. Always ease out with `ease-out-expo` (`cubic-bezier(0.16, 1, 0.3, 1)`). Under `prefers-reduced-motion`, every transition becomes an instant change or a crossfade.

## Do's and Don'ts

### Do:
- **Do** let artwork carry the colour, and keep chrome in pure greys up to `primary` (The One Voice Rule).
- **Do** keep solid `primary` to the one primary action on a screen; mark anything selected with the lit state.
- **Do** show Request state wherever the title appears, as a label and icon plus colour, never colour alone.
- **Do** use a scrim under any text that sits over artwork, so it keeps AA contrast.
- **Do** give every interactive element a visible focus: a 2px `ring` 2px off the edge, or a Button's or Input's rim and 3px ring.
- **Do** give every transition a `prefers-reduced-motion` fallback (a crossfade or an instant change).
- **Do** use exponential ease-out curves for motion. No bounce, no elastic, nothing that loops.

### Don't:
- **Don't** build **Plex's busy home**: no rows of promoted content, no upsells for services, no sidebars full of things you don't own.
- **Don't** build a **Netflix clone**: no generic black and red, no hero carousels, no autoplaying trailers.
- **Don't** show members the ***arr / admin-panel look**: no Bootstrap-ish tables, dense settings forms or developer aesthetic outside admin views.
- **Don't** repeat **Jellyfin's dated chrome**: no heavy cards, no inconsistent spacing, no settings sprawl.
- **Don't** put an `input` rim on `muted`, on `secondary` over `card` or on `primary-tint`, or `foreground-soft` on `secondary` over artwork (The Banned Pairings Rule).
- **Don't** use `muted-foreground` on `muted`, `secondary` or `primary-tint`.
- **Don't** add a second typeface, gradient text, glassmorphism or coloured side-stripe borders.
- **Don't** use tiny tracked-uppercase eyebrows above sections.
