/** `ListUnmatched`'s first page of the Library `libraryId`'s Unmatched Items. */
export function firstUnmatchedPage(libraryId: bigint) {
  return { libraryId, page: { token: "" } };
}
