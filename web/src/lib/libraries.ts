import { Code } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";

import { type FormErrors, refusedFieldErrors, refusal, serverMessage } from "./forms";

/** The query key of the Libraries, and of each one, to refetch after a change to them. */
export const librariesKey = createConnectQueryKey({
  schema: LibraryService,
  cardinality: "finite",
});

export function kindName(kind: LibraryKind) {
  return kind === LibraryKind.SHOWS ? "Shows" : "Movies";
}

/** The folders typed one per line, without blank lines or stray spaces. */
export function foldersTyped(typed: string) {
  return typed
    .split("\n")
    .map((folder) => folder.trim())
    .filter((folder) => folder !== "");
}

/** Why the Server refused to `attempt` (such as "create the Library"), next to its field. */
export function libraryErrors(error: unknown, attempt: string): FormErrors {
  return refusal(error, attempt, {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "folders"]),
    [Code.AlreadyExists]: { folders: serverMessage(error) },
    [Code.NotFound]: { form: "This Library isn’t on the Server any more." },
  });
}
