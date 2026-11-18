# Every User sees a Library's Unmatched Items, but not their paths

ADR 0013 says an Unmatched Item "can be played, but nothing is remembered until it is Matched". Until now no User could start one: Browse listed only Matched items, and the one list of Unmatched Items, `MatchService.ListUnmatchedItems`, needs Manage Libraries and gives no Version to play. A User whose files didn't Match saw nothing of them, which is the trap Jellyfin switchers fall into first (X13 in `docs/research/jellyfin-feature-research.md`).

So every signed-in User sees each Library's Unmatched Items, through `BrowseService.ListUnmatched`, and can play them by the Version that holds them. A member sees what each file's name was parsed as (title, year, Season and Episode) and its Version id, and never the absolute path, which says where a Library lives on disk and stays on Needs a Match for Users with Manage Libraries (`crates/nama-server/AGENTS.md`, "Clients name ids"). The web app shows them in a collapsed "Not matched" section at the bottom of the Library's page, only when it has some; Needs a Match also gets Play.

The list runs most recently found first, its keyset on when a scan found the item and its id, since a page token carries its key in the clear and a path there would hand members what the list leaves out (ADR 0055).

## Considered Options

- **Play only from Needs a Match, for Users with Manage Libraries**: rejected. It leaves every other User unable to play a file the Server holds, and #919's note for them would never show.
- **Unmatched Items stay unplayable, amending ADR 0013**: rejected. It keeps the X13 trap and goes back on a decision playback was built for (ADR 0035).
- **Members see paths too**: rejected. A path says where a Library lives on disk, which only Users with Manage Libraries see.

## Consequences

- The Server serves and probes the file of any Version holding an Item, Matched or not; a file holding no Item, one whose name didn't parse, is still never served.
- Playing an Unmatched Item keeps no Watch State and no Track Preference of its own (ADR 0013, ADR 0035).
