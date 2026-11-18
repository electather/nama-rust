/** What a page's URL says a delete left it to report: what went, and the Fulfillers not told. */
export interface DeletedSearch {
  deleted?: string | undefined;
  notTold?: string[] | undefined;
}

/**
 * The search that reports deleting `deleted` when the Fulfillers `notTold` couldn't be told, or
 * clears an earlier report when every one was.
 */
export function deletedSearch(deleted: string, notTold: string[]): DeletedSearch {
  return notTold.length > 0 ? { deleted, notTold } : { deleted: undefined, notTold: undefined };
}

/** The report of a delete in `search`, as `deletedSearch` wrote it. */
export function deletedFromSearch({ deleted, notTold }: Record<string, unknown>): DeletedSearch {
  // Set even when unknown: the router keeps the raw value under any key this leaves out.
  const unknown = { deleted: undefined, notTold: undefined };
  return typeof deleted === "string" && isNames(notTold) ? { deleted, notTold } : unknown;
}

function isNames(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((name) => typeof name === "string");
}
