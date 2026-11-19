---
name: Nama
description: A self-hosted media server with Requests built in. Quiet chrome, loud artwork.
colors:
  primary: "oklch(0.971 0.003 286.4)"
  on-primary: "oklch(0 0 0)"
  background: "oklch(0.164 0 0)"
  card: "oklch(0.213 0 0)"
  popover: "oklch(0.252 0 0)"
  muted: "oklch(0.309 0 0)"
  secondary: "oklch(0.715 0 0 / 15%)"
  foreground: "oklch(0.946 0 0)"
  muted-foreground: "oklch(0.65 0 0)"
  border: "oklch(0.715 0 0 / 15%)"
  input: "oklch(0.538 0 0)"
  destructive: "oklch(0.702 0.189 22.2)"
  scrim: "oklch(0 0 0 / 70%)"
typography:
  display:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "2.5rem"
    fontWeight: 500
    lineHeight: 1.05
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "1.875rem"
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: "-0.025em"
  title:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: 1.55
  body:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.75
  label:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.33
rounded:
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "14px"
  2xl: "18px"
  3xl: "22px"
  full: "9999px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.full}"
    padding: "6px 10px"
  button-secondary:
    backgroundColor: "{colors.secondary}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.full}"
    padding: "6px 10px"
  card:
    backgroundColor: "{colors.card}"
    rounded: "{rounded.xl}"
    padding: "18px"
---

# Design System: Nama

## 1. Overview

**Creative North Star: "The Dimmed Living Room"**

Nama should feel like a living room with the lights turned down before the film starts. The interface is the room: calm, warm and out of the way. The posters, stills and backdrops are the screen, and they hold all the attention and almost all the colour. Like the Apple TV app, Nama puts artwork first, leaves generous space and keeps its chrome so quiet you stop noticing it. It's confident because it's restrained, not because it's loud.

Density is relaxed on member surfaces (browse, title, player, Requests) and gets tighter only inside admin views, which are progressively disclosed. Motion is responsive: it gives feedback and makes transitions continuous (poster to title page, player controls fading in and out), but nothing is choreographed for show.

The system rejects Plex's busy home screen with its promoted rows and upsells, the generic Netflix-clone look of black and red with autoplaying hero carousels, the *arr / admin-panel aesthetic of Bootstrap tables and dense forms, and Jellyfin's heavy cards and inconsistent spacing.

**Key Characteristics:**
- Artwork is the loudest element on every screen. The interface never competes with it.
- Restrained colour: pure black-and-grey neutrals, with near-white as the only emphasis.
- One clean sans (Geist) in several weights. No second family.
- Flat surfaces separated by tone, not by shadows or borders.
- Responsive motion that confirms and connects, and is never decorative.

## 2. Colors

Pure neutrals and nothing else. The artwork supplies the colour, so the interface doesn't have to. Every colour is one of shadcn's tokens, plus `--scrim` (ADR 0016); a tint is an opacity of a token (`bg-destructive/10`), never a token of its own.

### Primary
- **Near-white** (`oklch(0.971 0.003 286.4)`): the solid fill of the one primary action on a screen (Play, Resume, Request), with black ink (`on-primary`, 19:1) in semibold. It's also the ink of a lit state (a selected segment or chip, on Accent), seek handles, the filled part of progress, focus rings (`--ring`), and inline links, underlined. In code this is shadcn's `--primary`. shadcn's own `--accent` is the quiet neutral fill of hover and lit states, never this colour.

### Neutral
- **Living-room black** (`oklch(0.164 0 0)`): the page.
- **Card** (`oklch(0.213 0 0)`): panels one step above the page.
- **Popover** (`oklch(0.252 0 0)`): things that float, such as menus and dialogs.
- **Muted** (`oklch(0.309 0 0)`): hover, disabled fills, troughs and tracks. `--accent` is the same value, for lit states.
- **Secondary** (grey at 15%): the fill of secondary buttons and chips. It's translucent, so it holds up on the page, on a card and over scrimmed artwork.
- **Ink** (`oklch(0.946 0 0)`): text, including body copy.
- **Quiet ink** (`oklch(0.65 0 0)`): metadata and supporting text, on the page, a card or a popover (4.9:1 at worst). On Muted or Secondary it falls to 4.1:1, so text there is Ink.
- **Hairline** (grey at 15%): decorative borders and dividers, below 3:1 on purpose.
- **Rim** (`oklch(0.538 0 0)`, `--input`): the edge of every control, 3:1 or better on every surface. At 30% it's the fill of an Input.

### Destructive
- **Red** (`oklch(0.702 0.189 22.2)`): ink for something that can't be undone and for Declined and Failed, on its own 10% tint (4.8:1 or better), always with an icon and a word.

The neutrals have no chroma. A coloured background competes with the artwork, so the ramp is pure grey. There are no status colours besides Destructive: Pending, Available and the other states are told apart by their icon and word. The values live in shadcn's `.dark` tokens, so a light theme later only needs new `:root` values (ADR 0016).

### Named Rules
**The One Voice Rule.** Solid Near-white fill belongs to one element per screen, the primary action. Lit states, progress, focus rings and links are the only other places it appears.

**The Artwork Owns Colour Rule.** Interface chrome never uses saturated colour for decoration. Every hue comes from the media, except Destructive.

**The Never Colour Alone Rule.** Request states (Pending, Approved, Declined, Processing, Partially Available, Available, Failed) and Match status always pair any colour with a label and an icon.

## 3. Typography

**Display Font:** Geist Variable (with `sans-serif`), bundled through Fontsource
**Body Font:** the same family
**Label/Mono Font:** none planned

**Character:** one clean sans in several weights, the face shadcn is built around. Large titles go lighter (medium) and tight-tracked, so they feel calm rather than loud. Hierarchy comes from weight and size, not from a second family.

### Hierarchy
- **Display** (500, 2.5rem, 1.05, -0.035em): Movie and Show titles on title pages, set over backdrops, when they have no Title Logo.
- **Headline** (`text-3xl font-medium tracking-tight`): page headings, Library names and shelf headings such as Continue Watching.
- **Title** (`text-lg font-semibold`): the title of a list row, a dialog or a settings pane.
- **Body** (`text-base leading-7`): overviews and descriptions, capped at 65–75ch.
- **Interface** (`text-sm`, `font-semibold` when selected): control labels, menu items and settings rows. A poster's caption and a settings group heading are `text-base font-medium`.
- **Label** (`text-xs`, Quiet ink): metadata such as year, runtime, rating and Request state. Sentence case, not tracked uppercase.

Only Display is a token (`--text-display`); every other step is Tailwind's own scale, as shadcn writes it.

### Named Rules
**The One Family Rule.** Nama uses one typeface. Hierarchy comes from weight (regular, medium, semibold) and size, never from adding a display or mono face.

**The Legible Over Art Rule.** Text set over artwork always sits on a scrim or gradient that guarantees 4.5:1 contrast, whatever the image.

## 4. Elevation

Flat by default. Depth comes from tonal layering plus a hairline: each surface steps one notch along the neutral ramp and carries a 1px white-at-8% border. Shadows are reserved for elements that genuinely float above the content, such as the player controls overlay, menus and dialogs. The one step above the page is `--card`, things that float are `--popover` with `shadow-md` (menus) or `shadow-lg` (dialogs, the player bar), and text over artwork sits on the `--scrim` token.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat at rest. If a poster grid needs shadows to separate its items, the spacing is wrong.

## 5. Components

Only the app shell exists so far. The tokens above set the shape every component inherits:

- **Buttons**: pills, 14px medium text, 32px tall. Play and Resume at the top of a title page are larger: 44px tall, 16px text, with a filled ▶. An icon-only button is 40px square with a 10px radius. Primary is Near-white with black semibold ink, once per screen. Disabled is Muted with Quiet ink, never a faded colour. Secondary is the translucent Secondary fill with Ink. Destructive, the confirm button of a dialog that asks before something can’t be undone (revoking a Device, deleting a User), is red ink on a 10% red tint. Everything else is ghost: no fill until hover, then Muted.
- **Cards**: Card, 14px radius, a hairline border, 18px padding. No shadow.
- **Inputs**: the Rim at 30% as a fill, a full Rim border, 10px radius. Focus turns the border Near-white with a 3px ring at 50%.
- **Menus**: Popover, 14px radius, a hairline, `shadow-md`. Items are 10px-radius rows that turn Accent when highlighted.
- **Sliders and progress**: a Muted track with a Near-white fill; a slider's thumb is Near-white.
- **Posters**: 2:3, 10px radius, a hairline border. A 2px progress bar sits under a poster with Watch State.
- **Backdrop cards**: 16:9, 10px radius, a hairline border, with the title over the art in Headline on the scrim, or its Title Logo, up to 48px tall and 60% of the card wide, and a Label line under the card. A Library's says its kind. A Continue Watching card puts ▶ and the 2px progress bar under the title, and under the card which Episode and how much is left, such as "S1, E3 · 33m left" or "1h 12m left". Without a Backdrop, the box is plain Muted with the title in it.
- **Title page top**: a Movie's or Show's page opens on its Backdrop, full width under the app bar, scrolling away with the page. The art is clear above the title, so it shows, and the title sits on at least the scrim however many lines it takes, fading into the page at the bottom. The title is its Title Logo, up to 112px tall (144px from `sm`) and 80% wide (448px from `sm`), or else its title in Display, and either way the `<h1>`. Under the art go a Label line of what it is, its genres and its year and runtime (a Show: its first air year), then its Content Rating and, on a Movie, its badges (4K, HDR or Dolby Vision, SDH), each in a 6px-radius box with a 14% hairline. Then the Play or Resume pill, with a 2px progress bar and what's left beside Resume, then the overview in Body, clamped to two lines with a ghost "More" that turns to "Less". A Request page shares only the Backdrop and title, always as text.
- **Episode cards**: a Show's Episodes, in the Backdrop cards' grid. The Still is 16:9, 10px radius, a hairline border, with nothing over it, and without one the box is plain Muted. Under it go a Label naming the Episode ("Episode 3", "Specials, Episode 3"), the title in Title, the synopsis in Label clamped to three lines, a Label line of air date, runtime and the Version's picture and sound, the 2px progress bar once started, then the played toggle and Delete.
- **State labels**: pill, Secondary fill, Ink, with an icon. Declined and Failed are red on a 10% red tint instead. No other state has a colour.

## 6. Do's and Don'ts

### Do:
- **Do** let artwork carry the colour, and keep chrome in pure neutrals (The One Voice Rule).
- **Do** put the primary action (Play, or Request for a missing title) in solid Near-white, and keep it to one per screen.
- **Do** show Request state wherever the title appears, as a label and an icon, never colour alone.
- **Do** use a scrim under any text that sits over artwork, so it keeps AA contrast.
- **Do** give every transition a `prefers-reduced-motion` fallback (a crossfade or an instant change).
- **Do** use exponential ease-out curves for motion. No bounce, no elastic.

### Don't:
- **Don't** build **Plex's busy home**: no rows of promoted content, no upsells for services, no sidebars full of things you don't own.
- **Don't** build a **Netflix clone**: no generic black and red, no hero carousels, no autoplaying trailers.
- **Don't** show members the ***arr / admin-panel look**: no Bootstrap-ish tables, dense settings forms or developer aesthetic outside admin views.
- **Don't** repeat **Jellyfin's dated chrome**: no heavy cards, no inconsistent spacing, no settings sprawl.
- **Don't** add a second typeface, gradient text, glassmorphism or coloured side-stripe borders.
- **Don't** use tiny tracked-uppercase eyebrows above sections.
