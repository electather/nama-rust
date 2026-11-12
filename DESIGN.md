---
name: Nama
description: A self-hosted media server with Requests built in. Quiet chrome, loud artwork.
colors:
  primary: "oklch(0.714 0.171 155)"
  on-primary: "oklch(0 0 0)"
  background: "oklch(0.145 0 0)"
  surface-raised: "oklch(0.185 0 0)"
  muted: "oklch(0.25 0 0)"
  foreground: "oklch(0.985 0 0)"
  muted-foreground: "oklch(0.65 0 0)"
  border: "oklch(1 0 0 / 8%)"
  input: "oklch(1 0 0 / 14%)"
  destructive: "oklch(0.704 0.191 22.216)"
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
    fontSize: "1.5rem"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.025em"
  title:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 500
    lineHeight: 1.4
  body:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Geist Variable, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.4
rounded:
  md: "8px"
  lg: "10px"
  xl: "14px"
  full: "9999px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.lg}"
    padding: "9px 16px"
  button-secondary:
    backgroundColor: "{colors.foreground}"
    textColor: "{colors.background}"
    rounded: "{rounded.lg}"
    padding: "9px 16px"
  card:
    backgroundColor: "{colors.surface-raised}"
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
- Restrained colour: pure black-and-grey neutrals plus a single green accent.
- One clean sans (Geist) in several weights. No second family.
- Flat surfaces separated by tone, not by shadows or borders.
- Responsive motion that confirms and connects, and is never decorative.

## 2. Colors

A restrained palette of pure neutrals with a single accent. The artwork supplies the colour, so the interface doesn't have to.

### Primary
- **Signal Green** (`#16C172`, `oklch(0.714 0.171 155)`): used only for the primary action on a screen (Play, Resume, Request), focus rings, and progress (Watch State bars, Request progress). It reads 8.4:1 on the page, so it carries black ink (`on-primary`, 8.9:1) in semibold, never white (2.4:1). It may also appear as a 14% tint behind a positive state label, such as an Available Request, always with its label and icon. In code this is shadcn's `--primary`, and `--ring` for focus. shadcn's own `--accent` is a quiet neutral hover background, never this colour (ADR 0016).

### Neutral
- **Living-room black** (`oklch(0.145 0 0)`, about `#0a0a0a`): the page.
- **Raised black** (`oklch(0.185 0 0)`): cards, popovers and `--surface-raised`, one step above the page.
- **Muted** (`oklch(0.25 0 0)`): hover backgrounds, inactive progress tracks.
- **Ink** (`oklch(0.985 0 0)`): text, and the secondary button's fill.
- **Quiet ink** (`oklch(0.65 0 0)`): metadata and supporting text, at least 4.9:1 on every surface.
- **Hairline** (white at 8%, inputs at 14%): borders and dividers.

The neutrals have no chroma. Both reference directions (shadcn, and dark product sites that keep one green) showed that a coloured background competes with the artwork, so the ramp is pure grey. The values live in shadcn's `.dark` tokens, so a light theme later only needs new `:root` values (ADR 0016).

### Named Rules
**The One Voice Rule.** The accent covers 10% of any screen at most. Solid accent fill belongs to one element per screen, the primary action. Focus rings, progress bars and a positive state tint are the only other places it appears.

**The Artwork Owns Colour Rule.** Interface chrome never uses saturated colour for decoration. Every hue that isn't the accent comes from the media.

**The Never Colour Alone Rule.** Request states (Pending, Approved, Declined, Processing, Partially Available, Available, Failed) and Match status always pair colour with a label or icon.

## 3. Typography

**Display Font:** Geist Variable (with `sans-serif`), bundled through Fontsource
**Body Font:** the same family
**Label/Mono Font:** none planned

**Character:** one clean sans in several weights, the face shadcn is built around. Large titles go lighter (medium) and tight-tracked, so they feel calm rather than loud. Hierarchy comes from weight and size, not from a second family.

### Hierarchy
- **Display** (500, 2.5rem, 1.05, -0.035em): Movie and Show titles on title pages, set over backdrops, when they have no Title Logo.
- **Headline** (600, 1.5rem, 1.2, -0.025em): page headings and Library names.
- **Title** (500, 0.9375rem, 1.4): section headings such as Continue Watching, poster captions and Request titles.
- **Body** (400, 1rem, 1.6): overviews and descriptions, capped at 65–75ch.
- **Label** (400, 0.8125rem, 1.4, Quiet ink): metadata such as year, runtime, rating and Request state. Sentence case, not tracked uppercase.

### Named Rules
**The One Family Rule.** Nama uses one typeface. Hierarchy comes from weight (regular, medium, semibold) and size, never from adding a display or mono face.

**The Legible Over Art Rule.** Text set over artwork always sits on a scrim or gradient that guarantees 4.5:1 contrast, whatever the image.

## 4. Elevation

Flat by default. Depth comes from tonal layering plus a hairline: each surface steps one notch along the neutral ramp and carries a 1px white-at-8% border. Shadows are reserved for elements that genuinely float above the content, such as the player controls overlay, menus and dialogs. The one step above the page surface is the `--surface-raised` token, and text over artwork sits on the `--scrim` token.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat at rest. If a poster grid needs shadows to separate its items, the spacing is wrong.

## 5. Components

Only the app shell exists so far. The tokens above set the shape every component inherits:

- **Buttons**: 10px radius, 14px medium text. Play and Resume at the top of a title page are a pill instead: full radius, 44px tall, 16px text, with a filled ▶. Primary is Signal Green with black semibold ink, once per screen. Disabled is Muted with Quiet ink, never a faded colour. Secondary is solid ink on black. Destructive, the confirm button of a dialog that asks before something can’t be undone (revoking a Device, deleting a User), is the destructive red with Living-room-black semibold ink. Everything else is ghost: no fill until hover, then Muted.
- **Cards**: Raised black, 14px radius, a hairline border, 18px padding. No shadow.
- **Inputs**: transparent, 14% hairline, 10px radius. Focus turns the border Signal Green with a 3px green ring at 25%.
- **Posters**: 2:3, 10px radius, a hairline border. A 2px progress bar sits under a poster with Watch State.
- **Backdrop cards**: 16:9, 10px radius, a hairline border, with the title over the art in Headline on the scrim, or its Title Logo, up to 48px tall and 60% of the card wide, and a Label line under the card. A Library's says its kind. A Continue Watching card puts ▶ and the 2px progress bar under the title, and under the card which Episode and how much is left, such as "S1, E3 · 33m left" or "1h 12m left". Without a Backdrop, the box is plain Muted with the title in it.
- **Title page top**: a Movie's or Show's page opens on its Backdrop, full width under the app bar, scrolling away with the page. It's clear at the top, so the art shows, and from 40% of the way down it's at least the scrim, fading into the page at the bottom, where the title sits. The title is its Title Logo, up to 112px tall (144px from `sm`) and 80% wide (448px from `sm`), or else its title in Display, and either way the `<h1>`. Under the art go a Label line of what it is, its genres and its year and runtime (a Show: its first air year), then its Content Rating and, on a Movie, its badges (4K, HDR or Dolby Vision, SDH), each in a 6px-radius box with a 14% hairline. Then the Play or Resume pill, with a 2px progress bar and what's left beside Resume, then the overview in Body, clamped to two lines with a ghost "More" that turns to "Less". A Request page has the same top with its title as text.
- **Episode cards**: a Show's Episodes, in the Backdrop cards' grid. The Still is 16:9, 10px radius, a hairline border, with nothing over it, and without one the box is plain Muted. Under it go a Label naming the Episode ("Episode 3", "Specials, Episode 3"), the title in Title, the synopsis in Label clamped to three lines, a Label line of air date, runtime and the Version's picture and sound, the 2px progress bar once started, then the played toggle and Delete.
- **State labels**: pill, hairline border, Quiet ink, with an icon. Only a positive state may use the green tint.

## 6. Do's and Don'ts

### Do:
- **Do** let artwork carry the colour, and keep chrome in pure neutrals with a single accent (The One Voice Rule).
- **Do** put the primary action (Play, or Request for a missing title) in the accent colour, and keep it to one per screen.
- **Do** show Request state wherever the title appears, as a label plus colour, never colour alone.
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
