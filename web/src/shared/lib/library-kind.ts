import { LibraryKind } from "@/gen/nama/v1/library_pb";

/** What a Library of `kind` holds, as people read it. */
export function kindName(kind: LibraryKind) {
  return kind === LibraryKind.SHOWS ? "Shows" : "Movies";
}
