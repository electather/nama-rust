// Resolving `value` against a stand-in origin keeps only paths on this Server, so a crafted
// `?redirect=` can't send someone elsewhere after they sign in.
const standIn = "http://nama.invalid";

function pathOnThisServer(value: unknown) {
  const url = typeof value === "string" ? URL.parse(value, standIn) : null;
  return url?.origin === standIn ? `${url.pathname}${url.search}${url.hash}` : undefined;
}

/** A route's `validateSearch` for `redirect`, where the browser goes once it's done, kept only as a path on this Server. */
export function redirectSearch(search: Record<string, unknown>): { redirect?: string | undefined } {
  // The router merges this over the raw search, so a refused `redirect` has to be overwritten.
  return { redirect: pathOnThisServer(search["redirect"]) };
}
