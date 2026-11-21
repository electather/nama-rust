# A Search source's titles count only by an External ID this Server knows

A Search source (#1460) adds its own batch to `SearchService.Search` (ADR 0080). It returns titles by External ID, and each may carry several, at most one per Namespace. The Kernel matches each title only against what this Server already knows. If any of its External IDs matches a title in a Library, an open Request or a Followed Show, the title is Available or Requested. If none matches, it is Not on this Server only when an enabled title source keeps one of its Namespaces and the caller may request that kind. Otherwise it is dropped, and nothing looks it up in another Namespace. Similar titles follow the same rule (ADR 0081). Search and Similar titles agree, a Plugin can never mislabel a title, and every title shown can be opened or requested.

## Considered Options

- **Show every title, marking unknown ones "Not on this Server"**: rejected. A title that no title source can resolve can't be requested, so the row would lead nowhere.
- **Drop every title no Library has (ADR 0081 applied strictly)**: rejected. A meaning search could then only re-rank what the Server already has, and couldn't surface a new title to request.
- **Look the title up in another Namespace through a title source**: rejected. That adds a fetch to every search, which ADR 0081 avoided for the same reason.

## Consequences

- Available and Requested titles use the Server's own Poster, `library_id` and `request`. A Not on this Server title uses the Plugin's title and year, and its Poster path is served through the Plugin's `title_image` export. Clients never contact an outside host.
- Batches arrive in a fixed order: Libraries, then the title source, then each Search source in the Admin's precedence. Within a Search source's batch, titles sent in an earlier batch are removed, then titles are ordered by status, then in the source's own order. The word-match bands don't apply. Each batch is cut to the request's `limit`, and the source is not asked again to fill it.
- Each source has one fixed Kernel timeout, with no Admin setting. A source that errors or runs past it is named in `Done`, and its batch is left out.
- A Search source receives only the query, the kind and the limit, never the User. It returns Movies and Shows, never Episodes. The Kernel asks it for every query that isn't blank. Enabling it is the Admin's consent for queries leaving the Server (ADR 0049).
