/** The folders typed one per line, without blank lines or stray spaces. */
export function foldersTyped(typed: string) {
  return typed
    .split("\n")
    .map((folder) => folder.trim())
    .filter((folder) => folder !== "");
}

/** `folders` with `picked` as a new last line, or `folders` itself if it's one of them already. */
export function foldersWith(folders: string[], picked: string) {
  return folders.includes(picked) ? folders : [...folders, picked];
}

/** What a Library's folders field asks for. */
export const foldersHint =
  "Where its files are on the Server: one full path per line, such as /srv/media/films.";
