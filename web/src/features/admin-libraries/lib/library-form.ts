/** The folders typed one per line, without blank lines or stray spaces. */
export function foldersTyped(typed: string) {
  return typed
    .split("\n")
    .map((folder) => folder.trim())
    .filter((folder) => folder !== "");
}

/** What a Library's folders field asks for. */
export const foldersHint =
  "Where its files are on the Server: one full path per line, such as /srv/media/films.";
