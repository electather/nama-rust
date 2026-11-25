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
  subtitle-ink: "oklch(1 0 0)"
  subtitle-edge: "oklch(0 0 0)"
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
  log:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.43
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

The Hub's web pages (Accounts, Linking and the Server list) build to this same system, from the same nama-ui tokens and components: each is one Card in the middle of the page, under the name "Nama Hub", as the web app's signed-out pages are.

Density is relaxed on member surfaces (browse, title, player, Requests) and gets tighter only inside admin views, which are progressively disclosed. Motion is responsive: it gives feedback and makes transitions continuous (poster to title page, player controls fading in and out), but nothing is choreographed for show.

The system rejects Plex's busy home screen with its promoted rows and upsells, the generic Netflix-clone look of black and red with autoplaying hero carousels, the *arr / admin-panel aesthetic of Bootstrap tables and dense forms, and Jellyfin's heavy cards and inconsistent spacing.

**Key Characteristics:**
- Artwork is the loudest element on every screen. The interface never competes with it.
- Restrained colour: pure black-and-grey neutrals, with near-white as the only emphasis.
- One clean sans (Geist) in several weights. No second family.
- Flat surfaces separated by tone, not by shadows or borders.
- Responsive motion that confirms and connects, and is never decorative.

## 2. Colors

Pure neutrals and nothing else. The artwork supplies the colour, so the interface doesn't have to. Every colour is one of shadcn's tokens, plus `--scrim` (ADR 0016) and the white `--subtitle-ink` and black `--subtitle-edge` of text subtitles, which sit over the video in every theme (ADR 0070); a tint is an opacity of a token (`bg-destructive/10`), never a token of its own.

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
- **Red** (`oklch(0.702 0.189 22.2)`): ink for something that can't be undone and for Declined and Failed, on its own 10% tint (4.8:1 or better), 15% on a state label (4.9:1 or better on the page and a card) except over a popover, where it keeps the 10% tint, always with an icon and a word.

The neutrals have no chroma. A coloured background competes with the artwork, so the ramp is pure grey. There are no status colours besides Destructive: Pending, Available and the other states are told apart by their icon and word. The values live in shadcn's `.dark` tokens, so a light theme later only needs new `:root` values (ADR 0016).

### Named Rules
**The One Voice Rule.** Solid Near-white fill belongs to one element per screen, the primary action. Lit states, progress, focus rings and links are the only other places it appears.

**The Artwork Owns Colour Rule.** Interface chrome never uses saturated colour for decoration. Every hue comes from the media, except Destructive.

**The Never Colour Alone Rule.** Request states (Pending, Approved, Declined, Processing, Partially Available, Available, Failed) and Match status always pair any colour with a label and an icon.

## 3. Typography

**Display Font:** Geist Variable (with `sans-serif`), bundled through Fontsource
**Body Font:** the same family
**Label/Mono Font:** the platform's monospace (Tailwind's `font-mono`), for log lines only

**Character:** one clean sans in several weights, the face shadcn is built around. Large titles go lighter (medium) and tight-tracked, so they feel calm rather than loud. Hierarchy comes from weight and size, not from a second family.

### Hierarchy
- **Display** (500, 2.5rem, 1.05, -0.035em): Movie and Show titles on title pages, set over backdrops, when they have no Title Logo.
- **Headline** (`text-3xl font-medium tracking-tight`): page headings, Library names and shelf headings such as Continue Watching.
- **Title** (`text-lg font-semibold`): the title of a dialog, or the heading of a section of a page, such as Cast & Crew. Settings is quieter: see Components. A Plugin's name at the top of its own view is `text-lg font-medium`.
- **Body** (`text-base leading-7`): overviews and descriptions, capped at 65–75ch.
- **Interface** (`text-sm`, `font-medium` when selected): control labels, menu items, and settings rows. Inside Settings it's 13px. A settings row's name is regular, and a settings group heading is regular Label in Quiet ink. A poster's caption is `text-base font-medium`.
- **Label** (`text-xs`, Quiet ink): metadata such as year, runtime, rating and Request state. Sentence case, not tracked uppercase.
- **Log** (`font-mono`, `text-sm`): what a line of a Plugin Log says, under its time in `font-mono text-xs` Quiet ink. Its level stays in the sans, with an icon.

Only Display is a token (`--text-display`); every other step is Tailwind's own scale, as shadcn writes it.

### Named Rules
**The One Family Rule.** Nama uses one typeface. Hierarchy comes from weight (regular, medium, semibold) and size, never from adding a display or mono face. The one exception is a log line: its time and its text are set in the platform's monospace, because they're read as data, with ids, URLs and stack traces that line up from one line to the next. Nothing else is set in it.

**The Legible Over Art Rule.** Text set over artwork always sits on a scrim or gradient that guarantees 4.5:1 contrast, whatever the image.

## 4. Elevation

Flat by default. Depth comes from tonal layering plus a hairline: each surface steps one notch along the neutral ramp and carries a 1px white-at-8% border. Shadows are reserved for elements that genuinely float above the content, such as the player controls overlay, menus and dialogs. The one step above the page is `--card`, things that float are `--popover` with `shadow-md` (menus) or `shadow-lg` (dialogs, the player bar), and text over artwork sits on the `--scrim` token.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat at rest. If a poster grid needs shadows to separate its items, the spacing is wrong.

## 5. Components

Only the app shell exists so far. The tokens above set the shape every component inherits:

- **Buttons**: pills, 14px medium text, 32px tall. Play and Resume at the top of a title page are larger: 44px tall, 16px text, with a filled ▶. An icon-only button (`icon-sm`) is 32px square with a 10px radius and a 16px icon, a dialog's ✕ included. Over video, the player's icon-only controls (`icon`) are 40px with a 20px icon. Primary is Near-white with black semibold ink, once per screen. Disabled is Muted with Quiet ink, never a faded colour. Secondary is the translucent Secondary fill with Ink. Destructive, the confirm button of a dialog that asks before something can’t be undone (revoking a Device, deleting a User), is red ink on a 10% red tint. Everything else is ghost: no fill until hover, then Muted.
- **Settings**: a dialog 700px wide (800px from `lg`, 1024px from `xl`) and at most 500px tall (90% of the screen's height from `xl`); full screen on a phone. Its sections run the full height down the side on the Sidebar surface, with Search settings at the top and the groups' names in Label. While the search has text, its results take the sections' place, each a Label Section name over the setting in Interface, the typed letters in `font-medium` and underlined; on a phone the field sits over the strip of sections, and the results take the pane's place. The row a result opens is lit Accent for a moment. Over the pane, its title is a trail in Interface: "Settings", a chevron, then the open section in `font-medium`, with the ✕ at the end. Settings is denser than the rest of the app: its Interface text is 13px, and wider than a phone its search, filter fields, choices in one pill and sections are 28px tall. Panes keep 20px at their sides and 16px between groups. A row keeps 6px over and under, with a 24px logo or icon tile at its start, centred on the row; on a phone a row's text field drops under its label and fills the row; nothing in Settings is semibold. A list a User puts in order, such as Home's rows, shows a row's arrows only on hover or focus (always on a touch screen); the row dragged lifts onto Popover with `shadow-lg` and follows the pointer, the rows it passes slide out of its way, and a row moved or dropped glides to its place on ease-out-expo. A row switched off dims its name and tile.
- **Cards**: Card, 14px radius, a hairline border, 18px padding. No shadow.
- **Inputs**: the Rim at 30% as a fill, a full Rim border, 10px radius, 32px tall with 10px at each side. A field that filters a list draws its 16px search icon inside that box. Choices in one pill (a segmented control) are 32px tall too, the chosen one raised on the Rim. On a phone both are 44px tall, so they're easy to tap. Focus turns the border Near-white with a 3px ring at 50%.
- **Menus**: Popover, 14px radius, a hairline, `shadow-md`. Items are 10px-radius rows that turn Accent when highlighted.
- **Sliders and progress**: a Muted track with a Near-white fill; a slider's thumb is Near-white. A Request's pill fills with Ink at 25% instead, so its label reads over it.
- **Posters**: 2:3, 10px radius, a hairline border. A 2px progress bar sits under a poster with Watch State.
- **Backdrop cards**: 16:9, 10px radius, a hairline border, with the title over the art in Headline on the scrim, or its Title Logo, up to 48px tall and 60% of the card wide, and a Label line under the card. A Library's says its kind. A Continue Watching card shows its titled Backdrop, with no title or Title Logo over it, only when it has no plain Backdrop: a 4:3 card crops a titled Backdrop's title. It puts ▶ and the 2px progress bar at the bottom of the card, under the title if it's drawn, and under the card which Episode and how much is left, such as "S1, E3 · 33m left" or "1h 12m left". Without a Backdrop, the box is plain Muted with the title in it. Continue Watching's cards share the Episode cards' frame: 4:3, 18px corners, the bottom blurred and darkened toward the scrim, the caption at the bottom start and the "…" at the end of its last line.
- **Title page top**: a Movie's or Show's page opens on its Backdrop, full width from under the app bar, at least 46rem tall, scrolling away with the page. The scrim is darkest at the bottom, where the Backdrop fades into the page, and at the start, where the copy sits, so the art shows clear above and beside it. At the bottom start, up to 560px wide: the title, its Title Logo up to 112px tall (144px from `sm`) and 80% wide (448px from `sm`), or else its title in Display, and either way the `<h1>`; a line of what it is and its genres; the overview in Body, clamped to two lines with a ghost "More" that turns to "Less"; "Starring" and the first three of its Cast in 14px Ink at 80%, each name an underlined link to its Person's page, left out without a Cast; a Label line of its Score as a filled star and one decimal (★ 8.4, left out when nobody has voted), its year and runtime (a Show: its first air year and how many Seasons), then its Content Rating and badges (4K, HDR or Dolby Vision, Atmos, SDH), each in a 6px-radius box with the Rim. Then the actions in a row: the 44px Play or Resume pill, with a 2px progress bar and what's left in Resume, or on a page to Request, Request, as tall, and the Request's pill; the Watchlist's round button; and a round "…" for what else may be done (the Version Play plays, marking it watched, Edit images, Delete). A Request page's title is always text.
- **Cast & Crew**: on a title's page, above its Details, a Title heading over its Cast, then its Crew, in one row that scrolls sideways out to the screen's edges as the Episode row does. Each is 96px wide: a round headshot with the Rim, or a plain Muted circle with a person icon, over the name in medium and the character or job in Muted ink, each up to two lines. Each opens its Person's page. A title without Roles has no row.
- **Person page**: a Person's headshot, as Cast & Crew draws it, beside their name as the page heading, then "Cast" and "Crew", each a Title heading over the titles on this Server they have a Role in, newest first, as a Library's posters: the name, then a Label line of the year and their character or job. A list with no titles is left out.
- **Trailers**: above a title's Details, under a Title heading "Trailers", its Trailers as Secondary buttons 44px tall that wrap, each its name and an arrow out, opening where it's hosted in a new tab. Nothing from the host is drawn on the page, no thumbnail or player, so showing a title tells it nothing; a title without Trailers leaves the row out.
- **Title Details**: the foot of a title's page, under a hairline: up to three columns, "Information", "Languages" and "On this Server", each a Title heading over its facts, a Label name over an Ink value at 80%. Information ends with who directed a Movie ("Directed by") or created a Show ("Created by"), each name an underlined link to its Person's page. A fact the Server doesn't know is left out, and a column with none.
- **Season menu**: on a Show's page, over one row of every Episode, Season after Season, Specials last, the Season the row is on as a Secondary button, 44px tall, its name in 18px semibold and a ▾. Its menu lists each Season by name alone; picking one scrolls the row to its first Episode, and scrolling the row changes the button to the Season at its start. A Show with one Season shows its name as plain text. The row keeps only the cards near the view in the page, and snaps by proximity, so a jump far along it lands where it was sent.
- **Episode cards**: a Show's Episodes, in one row that scrolls sideways past the page's column to the screen's edges, snapping to each card, drawn as Continue Watching's cards are (one shared card): the Still fills a 4:3 card with 18px corners, square when it has a synopsis, so the art still shows above it and a hairline, its bottom blurred and darkened toward the scrim, and over it at the bottom start a Label naming the Episode ("Episode 3", "Specials, Episode 3"), the title in semibold, the synopsis clamped to three lines, then its runtime, "✓ Watched" once played, or once started ▶, a 4px bar of how far in and what's left. The card's "…", at the end of that last line, marks it watched or deletes it. Without a Still, the card is plain Muted.
- **Similar titles row**: a title's Similar titles that are in a Library, above its Details, under an 18px semibold heading: "Similar titles" on its page, "On this Server" on its page to Request. Posters with their caption, in one row that scrolls sideways past the page's column to the screen's edges, snapping to each. Left out when none is in a Library.
- **State labels**: pill, Secondary fill, Ink, with an icon, at every size. Something that gave way or waits on the User, such as a Playback Limit the player couldn't keep or a Plugin Connection waiting for you, is drawn the same, its icon and word telling it apart. A state that's as it should be, such as Connected or Password changed, is lit: Accent fill, Near-white ink. Declined, Failed and a setting not saved are red on a 15% red tint instead, 10% over a popover such as the Save toast. No other state has a colour.
- **Request pills**: a title shows its Request as a state label that says what's left for whoever asked: Pending reads "Pending approval", Approved "Requested", and Processing with a download "Downloading", filled to its percent with the time left. Failed gives its reason on hover and on focus. Beside a title's Play it's as tall, 44px with 16px text. The Requests pages keep the Glossary's words.

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
