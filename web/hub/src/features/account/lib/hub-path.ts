/**
 * `value` if it's a path on the Hub, which signing in may go on to. Anything else, such as
 * `//elsewhere.example`, which a browser reads as another site, is dropped.
 */
export function hubPath(value: unknown) {
  return typeof value === "string" && value.startsWith("/") && !/^\/[/\\]/u.test(value)
    ? value
    : undefined;
}
