/** What a Movies Library's page shows: its Movies, or its Box Sets. */
export type LibraryView = "movies" | "box-sets";

/** The view a Library's URL names in its search parameters: none for the Movies. */
export interface ViewSearch {
  view?: "box-sets" | undefined;
}

/** The view `search` names, as the URL's `view`: the Box Sets, or else the Movies. */
export function viewFromSearch({ view }: Record<string, unknown>): ViewSearch {
  return { view: view === "box-sets" ? "box-sets" : undefined };
}

/** The first page of `ListBoxSets` for the Library `libraryId`, 100 a page as its Movies are. */
export function firstBoxSetPage(libraryId: bigint) {
  return { libraryId, page: { size: 100, token: "" } };
}
