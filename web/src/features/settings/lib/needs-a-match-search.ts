import { detailSearch, sectionSearch } from "@/shared/lib/settings-address";

/**
 * The words of Needs a Match's list, filtered to the Library `library` if one is, to spread over
 * the page's search. As a link to a Section does, they leave Settings' own history alone.
 */
export function listSearch(library: string | undefined) {
  return library === undefined
    ? sectionSearch("needs-a-match")
    : detailSearch({ kind: "library-filter", id: library });
}
