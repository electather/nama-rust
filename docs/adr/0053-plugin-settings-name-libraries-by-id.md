# Plugin settings name Libraries by id

The Cleanup Plugin took the Libraries it cleans as a Text setting of names, and remembered which ids each name had resolved to so a renamed Library stayed in. That fixed the symptom, not the cause: a Library renamed before any tick saw it was dropped silently, and a name another Library took over replaced the remembered ids by a guess. A setting that names a Library must hold what never changes, its id.

Plugin settings gain a kind for it, `SETTING_KIND_LIBRARIES`, in both the Plugin contract and the client API. Its value is `SettingValue.library_ids`, a `LibraryIds` holding the ids in the order the admin gave them, which the Plugins page fills from the Admin's Library list.

- **The Kernel checks the ids when settings are saved.** `UpdatePluginSettings` refuses an id no Library has with `REFUSAL_REASON_LIBRARY_NOT_FOUND`, naming the setting and the id, and saves nothing.
- **A Library deleted later stays in the value.** The Kernel doesn't rewrite a Plugin's settings when a Library goes: it stops listing the Library, and the Plugin ignores an id it no longer finds. Enabling a Plugin whose saved setting names a deleted Library still works.
- **An Installed Plugin reads the ids as text**, in decimal joined by `,` and empty for none, in its Extism config like any setting; `nama_pdk::setting_library_ids` reads them back.
- **The kind is never a Personal setting.** A User can't pick from the Admin's Library list, so the Kernel refuses a Manifest whose `personal_settings` use it, as it does URL and CHOICE (ADR 0046).
- **Cleanup moves to the kind.** Its free-text name setting and the name-to-id memory in `plugins/cleanup` are dropped. Nothing is kept compatible: an Admin who had Cleanup set up picks its Libraries again.

## Considered Options

- **Keep names and remember the ids they resolved to**: rejected, for the two failures above.
- **Ids typed as text or an INTEGER list**: rejected. The Admin would have to look ids up, and the Kernel couldn't check them or offer a picker.
- **Drop a deleted Library's id from every Plugin's settings**: rejected. Deleting a Library would then change Plugins' settings, which only `PluginSettings` writes (ADR 0028), and a Plugin that skips unknown ids needs no such write.

## Consequences

- `CONTRACT_VERSION` 0.4.0 covers the kind, in the one bump shared with the blob store point (ADR 0052).
- The client API's `SettingKind` and `SettingValue` grow a value, an addition rather than a break; the Plugins page gets a Library picker for it.
- A Plugin with a Libraries setting must skip ids the Kernel no longer lists.
