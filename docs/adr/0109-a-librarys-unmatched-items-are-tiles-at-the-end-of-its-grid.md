# A Library's Unmatched Items are tiles at the end of its grid

ADR 0069 has the web app show a Library's Unmatched Items in a collapsed "Not matched" section at the foot of its page. This replaces that sentence and nothing else: every User still sees them by what their names say, can play them, and never sees a path.

The Library's grid now ends with them as tiles, after its last title: a plain Muted poster box titled by the parsed name, with Play and, for a User with Manage Libraries, Find a Match always shown over its foot. A "Not matched" chip in the Filter panel, with how many there are, lists only the tiles (`?unmatched`); the chip turns off when a genre or Watch State is picked, and the tiles are left out while a title filter is on, since they match none.

## Considered Options

- **Keep the folded section:** rejected. Files that didn't Match sat out of sight below a long grid, where the one place a switcher looks for them, the Library, never said they were there.
- **Tiles among the titles, by name:** rejected. A title's place in the grid, and the quick-scroll rail's letters, say where it sorts; a file the Server couldn't name has no place in that order.
- **Show a count and a link to Needs a Match:** rejected. Needs a Match is for Users with Manage Libraries and lists paths.

## Consequences

- The grid pages through its titles first and then the Unmatched Items, counting both.
- With Not matched on, the quick-scroll rail is empty and the header counts "N Not matched".
