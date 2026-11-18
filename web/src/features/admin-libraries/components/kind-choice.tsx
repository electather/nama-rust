import type { Library } from "@/gen/nama/v1/library_pb";
import { Choices } from "@/shared/components/form-field/choices";

const kinds: [value: string, label: string][] = [
  ["movies", "Movies"],
  ["shows", "Shows"],
];

/** What a new Library, without `library`, holds, which can't change after. */
export function KindChoice({ library }: { library: Library | undefined }) {
  return library === undefined ? <Choices legend="Holds" name="kind" choices={kinds} /> : null;
}
