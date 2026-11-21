import type { Done, Hit, SearchResponse } from "@/gen/nama/v1/search_pb";

/**
 * The titles `Search` has streamed so far, in the order to list them: by status (Available,
 * Requested, then Not on this Server, as their numbers go), and in the Server's order within one.
 */
export function hitsIn(responses: readonly SearchResponse[]): Hit[] {
  return (
    responses
      .flatMap(({ part }) => (part.case === "hits" ? part.value.hits : []))
      // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
      .sort((a, b) => a.status - b.status)
  );
}

const sourceList = new Intl.ListFormat("en-GB", { type: "conjunction" });

/**
 * What `Search`'s `Done` said is missing from its results, in plain words, or "" if nothing is:
 * the titles to request, if the title source couldn't be searched, and the Search sources that
 * didn't answer.
 */
export function searchNote(responses: readonly SearchResponse[]): string {
  return responses
    .flatMap(({ part }) => (part.case === "done" ? missingFrom(part.value) : []))
    .join(" ");
}

/** A sentence for each thing `done` says is missing. */
function missingFrom({ titleSourceUnavailable, failedSearchSources }: Done): string[] {
  const missing = titleSourceUnavailable
    ? ["Titles to request couldn’t be searched, so only this Server’s are listed."]
    : [];
  return failedSearchSources.length === 0 ? missing : [...missing, unanswered(failedSearchSources)];
}

/** That the Search sources `failed`, by their Plugin ids, didn't answer. */
function unanswered(failed: string[]): string {
  const sources = failed.length === 1 ? "The Search source" : "The Search sources";
  return `${sources} ${sourceList.format(failed)} didn’t answer, so some titles may be missing.`;
}
