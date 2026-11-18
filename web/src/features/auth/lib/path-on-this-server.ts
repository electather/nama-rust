// Resolving `value` against a stand-in origin keeps only paths on this Server, so a crafted
// `?redirect=` can't send someone elsewhere after they sign in.
const standIn = "http://nama.invalid";

/** `value` as a path on this Server, or `undefined` when it points anywhere else. */
export function pathOnThisServer(value: unknown) {
  const url = typeof value === "string" ? URL.parse(value, standIn) : null;
  return url?.origin === standIn ? `${url.pathname}${url.search}${url.hash}` : undefined;
}
