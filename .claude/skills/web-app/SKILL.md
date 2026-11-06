---
name: web-app
description: Building, checking, screenshotting and reviewing the web app. Use when a change touches `web/`.
---

# Web app

`impeccable` below is the impeccable skill's launcher. When it isn't on `PATH`, run it as `~/.claude/skills/impeccable/scripts/impeccable`.

## Build

Follow `web/AGENTS.md`. Load the `impeccable` skill, read the references it points to, run its `context` step from the repo root, and build to `PRODUCT.md` and `DESIGN.md`.

## Check

From `web/`, run `vp install`, then every command in `web/README.md` "Checks", until each passes with no findings.

## Look

Before committing, screenshot every screen you changed and judge it against `DESIGN.md`:

1. Start `vp dev` in the background, fed by a fake transport if `web/README.md` documents a dev mode for one, or else by a Server you start yourself (`web/README.md`, "Running against a Server").
2. For each changed screen, run `playwright screenshot --viewport-size "390,844" <url> /tmp/<screen>-phone.png`, and again with `"1440,900"` for desktop.
3. Open each PNG and fix what departs from `DESIGN.md`.

Done when every changed screen matches `DESIGN.md` at both widths. Keep the screenshots in `/tmp`: they're your own check, not part of the change.

## Review

Audit the changed UI files against `PRODUCT.md` and `DESIGN.md` with the `impeccable` skill's audit (its `reference/audit.md`). The findings of `impeccable detect` and any breach of a `DESIGN.md` "Don't" are hard findings.
