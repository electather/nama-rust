/** The folders typed one per line, without blank lines or stray spaces. */
export function foldersTyped(typed: string) {
  return typed
    .split("\n")
    .map((folder) => folder.trim())
    .filter((folder) => folder !== "");
}

/** The title sources the form holds, in the order the Admin set. */
export function titleSourcesTyped(typed: FormData) {
  return typed.getAll("titleSources").filter((each) => typeof each === "string");
}

/** The ids of the Recogniser Plugins the form holds, in the order the Admin set. */
export function recognisersTyped(typed: FormData) {
  return typed.getAll("recognisers").filter((each) => typeof each === "string");
}
