import type { Hit, SearchResponse } from "@/gen/nama/v1/search_pb";

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

/** Whether `Search` said the titles to request couldn't be searched, so only this Server's are found. */
export function titleSourceUnavailable(responses: readonly SearchResponse[]): boolean {
  return responses.some(({ part }) => part.case === "done" && part.value.titleSourceUnavailable);
}
