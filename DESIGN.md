<!-- SEED: re-run /impeccable document once there's code to capture the actual tokens and components. -->
---
name: Nama
description: A self-hosted media server with Requests built in. Quiet chrome, loud artwork.
---

# Design System: Nama

## 1. Overview

**Creative North Star: "The Dimmed Living Room"**

Nama should feel like a living room with the lights turned down before the film starts. The interface is the room: calm, warm and out of the way. The posters, stills and backdrops are the screen, and they hold all the attention and almost all the colour. Like the Apple TV app, Nama puts artwork first, leaves generous space and keeps its chrome so quiet you stop noticing it. It's confident because it's restrained, not because it's loud.

Density is relaxed on member surfaces (browse, title, player, Requests) and gets tighter only inside admin views, which are progressively disclosed. Motion is responsive: it gives feedback and makes transitions continuous (poster to title page, player controls fading in and out), but nothing is choreographed for show.

The system rejects Plex's busy home screen with its promoted rows and upsells, the generic Netflix-clone look of black and red with autoplaying hero carousels, the *arr / admin-panel aesthetic of Bootstrap tables and dense forms, and Jellyfin's heavy cards and inconsistent spacing.

**Key Characteristics:**
- Artwork is the loudest element on every screen. The interface never competes with it.
- Restrained colour: tinted neutrals plus a single accent.
- One warm humanist sans in several weights. No second family.
- Flat surfaces separated by tone, not by shadows or borders.
- Responsive motion that confirms and connects, and is never decorative.

## 2. Colors

A restrained palette of tinted neutrals with a single accent. The artwork supplies the colour, so the interface doesn't have to.

### Primary
- **Accent** [hue and value to be resolved during implementation]: used only for the primary action on a screen (Play, Request), focus rings, and progress (Watch State bars, Request progress). Choose a hue that sits comfortably next to arbitrary poster art and still meets AA contrast on the neutral surfaces. In code this is shadcn's `--primary`, and `--ring` for focus. shadcn's own `--accent` is a quiet neutral hover background, never this colour (ADR 0016).

### Neutral
- **Surfaces, ink and dividers** [to be resolved during implementation]: a tonal ramp tinted 0.005–0.015 chroma toward the accent's hue. The theme is dark, chosen for the physical scene in PRODUCT.md: a couch, evening, often dim light. The values live in shadcn's `.dark` tokens, so a light theme later only needs new `:root` values (ADR 0016).

### Named Rules
**The One Voice Rule.** The accent covers 10% of any screen at most. If two things on a screen are accent-coloured and only one of them is the primary action, one of them is wrong.

**The Artwork Owns Colour Rule.** Interface chrome never uses saturated colour for decoration. Every hue that isn't the accent comes from the media.

**The Never Colour Alone Rule.** Request states (Pending, Approved, Declined, Processing, Partially Available, Available, Failed) and Match status always pair colour with a label or icon.

## 3. Typography

**Display Font:** [humanist sans to be chosen at implementation, from Google Fonts, bundled with the app through Fontsource]
**Body Font:** the same family
**Label/Mono Font:** none planned

**Character:** a single warm humanist sans used in several weights. It should read as friendly and calm, never technical. Hierarchy comes from weight and size, not from a second family.

### Hierarchy
- **Display**: Movie and Show titles on title pages, set over backdrops.
- **Headline**: Library names and section headings such as Continue Watching.
- **Title**: poster captions and Request titles.
- **Body**: overviews and descriptions, capped at 65–75ch.
- **Label**: metadata such as year, runtime, rating and Request state. Sentence case, not tracked uppercase.

### Named Rules
**The One Family Rule.** Nama uses one typeface. Hierarchy comes from weight (regular, medium, semibold) and size, never from adding a display or mono face.

**The Legible Over Art Rule.** Text set over artwork always sits on a scrim or gradient that guarantees 4.5:1 contrast, whatever the image.

## 4. Elevation

Flat by default. Depth comes from tonal layering: each surface steps one notch along the neutral ramp. Shadows are reserved for elements that genuinely float above the content, such as the player controls overlay, menus and dialogs. The one step above the page surface is the `--surface-raised` token, and text over artwork sits on the `--scrim` token.

### Named Rules
**The Flat-By-Default Rule.** Surfaces are flat at rest. If a poster grid needs shadows to separate its items, the spacing is wrong.

## 5. Components

[No components exist yet. They'll be documented on the first scan-mode run.]

## 6. Do's and Don'ts

### Do:
- **Do** let artwork carry the colour, and keep chrome in tinted neutrals with a single accent (The One Voice Rule).
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
