import type { Row } from "@/features/action-bar/lib/rows";
import type { FoundPerson } from "@/features/action-bar/lib/titles";
import { type Library, LibraryKind } from "@/gen/nama/v1/library_pb";
import { kindName } from "@/shared/lib/library-kind";

/** The People a search found, one row each. */
export function personRows(
  people: readonly FoundPerson[],
  openPerson: (personId: string) => void,
): Row[] {
  return people.map((person) => ({
    id: `person:${person.personId}`,
    label: person.name,
    hint: "",
    icon: "person",
    poster: person.headshot ?? null,
    action: "Open",
    run: () => {
      openPerson(person.personId);
    },
  }));
}

/** The Libraries to jump to, one row each. */
export function libraryRows(
  libraries: readonly Library[],
  openLibrary: (library: Library) => void,
) {
  return libraries.map((library): Row => ({
    id: `library:${String(library.id)}`,
    label: library.name,
    hint: `A Library of ${kindName(library.kind)}`,
    icon: library.kind === LibraryKind.SHOWS ? "show" : "movie",
    action: "Go",
    run: () => {
      openLibrary(library);
    },
  }));
}
