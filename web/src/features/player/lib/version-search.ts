/** A player's search params: the Version `?version=` chose, if it names one by id. */
export function versionSearch({ version }: Record<string, unknown>): { version?: number } {
  return typeof version === "number" ? { version } : {};
}
