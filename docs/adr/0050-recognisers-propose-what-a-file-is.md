# Recognisers propose what a file is; the Kernel decides

The Kernel's Matching reads a file's name and searches the Library's title sources. It can't read every naming scheme: an anime Episode numbered from the start of its Show, such as `Show - 137.mkv`, has no `SxxEyy` or `1x02`, so the name parser makes no Item from it and it is stored as an unparsed file the Matching never sees. A Recogniser is the Extension Point for that knowledge, `nama.plugin.v1.RecogniserService`, with the Built-in `Recogniser` trait mirroring it. It is named apart from the code's `matching::Matcher`, which fixes Matches by hand. Both of its methods are optional, behind the table-driven adapter (ADR 0043), so one point serves a Plugin that reads file names, one that knows titles, or both.

A Recogniser only proposes. The Kernel owns every Match, which Contents writes, and the Unmatched decision, and its own Matching stays the Built-in default it falls back to. It asks at two levels:

- **Title level** (`recognise_title`): for an Item, the Kernel tries a pin, then an ID Hint, then the title sources' search with its year-less retry. Then it asks each Recogniser in order with the Item's kind, name, year, paths and the search's final candidates. The first to propose ids with a namespace one of the Library's title sources serves wins, and the Kernel fetches the title as it does for an ID Hint. If none does, `confidence::decide`'s decision stands.
- **Unread files** (`recognise_file`): the Kernel asks each Recogniser in order about a file the name parser makes no Item from, with its path relative to the Library's folder and the Library's kind. The first proposal, a Movie with a title, year and any ids, or a Show's title, year, Season and Episode numbers, becomes Items the Kernel then Matches as usual.

An empty answer means the Recogniser can't tell, and the Kernel asks the next one. A Recogniser that fails leaves the Item or file undecided for the next Scan, as a failing title source does (ADR 0044), so a Recogniser being down never stores an Unmatched it would have decided otherwise.

The Kernel asks on a new or changed file, and once for a Library's Unmatched and unparsed files when its Recognisers change: one is enabled or the Library's order of them changes. Nothing else already matched is re-decided. Each Library holds its Recognisers in an order the Admin sets, stored and shown as its title source order is.

## Considered Options

- **One Extension Point per level**, a title recogniser and a file recogniser: rejected. Both levels serve the same knowledge of a naming scheme, and optional methods on one point let a Plugin serve either or both, as a title source serves only some of its methods.
- **Recognisers writing Matches**: rejected. The Kernel would lose the one place that decides a Match and what is Unmatched, a pin could be overwritten, and a failing or wrong Recogniser could leave a Library in a state no Admin chose.

## Consequences

- The contract adds `RecogniserService` 1.0.0 and `CONTRACT_VERSION` takes a patch bump; a Plugin built before it is unaffected.
- A Recogniser's title-level proposal is only as good as the Library's title sources: ids in a namespace none of them serves are ignored.
- Files already matched stay as they are when a Recogniser is enabled; only Unmatched and unparsed files are asked again.
