import type { Library } from "@/gen/nama/v1/library_pb";

/** The Libraries the filter `picked` shows: every one without it, none if it names no Library. */
export function pickedLibraries(libraries: Library[], picked: string | undefined) {
  if (picked === undefined) {
    return libraries;
  }
  const library = libraries.find(({ id }) => String(id) === picked);
  return library && [library];
}
