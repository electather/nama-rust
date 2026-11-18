/** Seconds as whole milliseconds, as the Server takes positions. */
export function toMs(seconds: number) {
  return BigInt(Math.round(seconds * 1000));
}
