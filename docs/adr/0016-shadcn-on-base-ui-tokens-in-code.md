# Web UI components are shadcn on Base UI, owned and restyled, with design tokens in code

The web app's components come from shadcn/ui, initialised with Base UI primitives (`shadcn init -t start -b base`) rather than Radix. Base UI is actively developed by the people behind Radix, Floating UI and MUI, while Radix has slowed. It also has accessible primitives M2 needs that Radix lacks or does weakly: Slider for the player, NumberField and Combobox. shadcn supports it first-class.

shadcn copies component source into `web/src/components/ui/`, and we own that copy. Every component is restyled to `DESIGN.md` (flat at rest, separated by tone, not borders or shadows) and added only when a screen needs it. Stock shadcn is the generic look that `DESIGN.md` and impeccable reject.

Colours are shadcn's semantic CSS variables, kept in one stylesheet, nama-ui's `web/ui/src/styles.css` (ADR 0101). Components use only the semantic utilities (`bg-background`, `text-muted-foreground`), never Tailwind palette colours or `dark:` variants, so a theme is just a set of token values. M2 is dark only: `<html>` always has `class="dark"`. Adding light mode means tuning the `:root` values and adding a toggle, not touching components. `--primary` is `DESIGN.md`'s Near-white, the one emphasis, and `--ring` follows it. shadcn's own `--accent` is the quiet fill of hover and lit states.

The code is the source of truth for the design system. `DESIGN.md` is regenerated from it with `impeccable document`, not the other way round.

## Considered Options

- **shadcn on Radix (the default)**: rejected for the maintenance and component reasons above.
- **A styled kit (MUI, Mantine, Chakra)**: rejected. Its look is hard to escape, and restyling it to `DESIGN.md` would mean fighting its theme system.
- **Design tokens of our own instead of shadcn's variable names**: rejected. Every shadcn component would need its classes rewritten when added.

## Consequences

- `styles.css` resets Tailwind's palette (`--color-*: initial`), so a raw palette class fails to exist rather than slipping through review.
- Beyond shadcn's set, the only extra token is `--scrim`: the palette is off, so nothing else can be black. A tint is an opacity of a token (`bg-destructive/10`), and type, spacing, radius and shadow use Tailwind's own steps, as shadcn's components do. A new token needs a screen that no shadcn token can serve (#1151).
- Re-running `shadcn add` on a component overwrites our restyle. Diff it before committing.
