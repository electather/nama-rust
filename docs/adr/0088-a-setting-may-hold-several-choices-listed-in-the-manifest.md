# A setting may hold several choices, listed in the manifest

Top Picks' Family Content Ratings was free text, so a typo matched nothing and an admin had to know which ratings exist (#1655). The Plugin contract gains `SETTING_KIND_CHOICES`: a setting holding several values, each picked from a list of choices. Unlike `SETTING_KIND_CHOICE`, whose choices come only from `check_settings()`, its choices can be listed on the `SettingField` in the manifest. When `check_settings()` returns choices for it, those replace the manifest's. A Plugin with a fixed list then needs no settings check, and one whose list depends on the Server, such as the Content Ratings its Libraries hold, can still offer that.

The value is a new `SettingValue` arm holding the picked choices' values. Each value is kept once, in the order the choices are listed. An empty list means none was picked, and a `required` setting counts it as missing. As for `SETTING_KIND_CHOICE`, a value that isn't offered is refused when settings are saved and never checked when the Plugin is enabled. A Plugin ignores a saved value it no longer offers, as it ignores a deleted Library's id.

## Considered Options

- **Choices only from `check_settings()`, like `SETTING_KIND_CHOICE`**: rejected. Every Plugin with a fixed list would need a settings check that only returns a constant.
- **Choices only in the manifest**: rejected. A list that depends on the Server, or on another setting, couldn't be offered.
- **Manifest choices for `SETTING_KIND_CHOICE` too**: not done yet. No Plugin needs them, and adding them later is additive.

## Consequences

- A Server refuses a Plugin that sets manifest choices on any other kind, repeats a choice value, has a default holding a value that isn't among its manifest choices, or has a `SETTING_KIND_CHOICES` setting with no manifest choices and no settings check.
- Personal settings refuse the kind, as they refuse `SETTING_KIND_CHOICE` (ADR 0046).
- The change is additive, so the Plugin contract version gets the bump an addition takes, a patch while it is 0.x (`nama-plugin-proto`'s `AGENTS.md`), and `proto/BREAKING.md` needs no entry (ADR 0041).
- An Installed Plugin reads the value as a JSON array of strings, since a choice's value may hold any text, so Library ids' `,`-joined form won't do.
