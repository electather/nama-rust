import { create } from "@bufbuild/protobuf";

import { SwitchRow } from "@/features/settings/components/switch-row";
import { type LibraryAccess, LibraryAccessSchema } from "@/gen/nama/v1/sign_in_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { useLibraries } from "@/shared/hooks/use-libraries";

const ALL = "all";
const CHOSEN = "chosen";

/**
 * The Libraries a User sees, `libraryAccess`, unset for every one, as a row and, for a chosen list,
 * a switch for each Library, each change going to `onChange`, to save at once.
 */
export function LibraryAccessField({
  libraryAccess,
  onChange,
}: {
  libraryAccess: LibraryAccess | undefined;
  onChange: (changes: { libraryAccess: LibraryAccess | undefined }) => void;
}) {
  // ponytail: lists only the Libraries the editor sees, which is every one for an Admin or a
  // User with Manage Libraries; upgrade when the Server lists them all for Manage Users (#2323).
  const libraries = useLibraries();
  const choose = (libraryIds: bigint[]) => {
    onChange({ libraryAccess: create(LibraryAccessSchema, { libraryIds }) });
  };
  return (
    <>
      <FormSelect
        row
        label="Libraries"
        hint="All Libraries takes in those added later. A User with Manage Libraries sees every one."
        name="libraryAccess"
        value={libraryAccess ? CHOSEN : ALL}
        onValueChange={(value) => {
          // Choosing starts from what they saw, every Library, for the Admin to take some away.
          if (value === ALL) {
            onChange({ libraryAccess: undefined });
          } else if (!libraryAccess) {
            choose(libraries.map(({ id }) => id));
          }
        }}
        options={[
          { value: ALL, label: "All Libraries" },
          { value: CHOSEN, label: "Chosen Libraries" },
        ]}
      />
      {libraryAccess && (
        <fieldset>
          <legend className="sr-only">Chosen Libraries</legend>
          {libraries.map(({ id, name }) => (
            <SwitchRow
              key={id}
              label={name}
              checked={libraryAccess.libraryIds.includes(id)}
              onChange={(checked) => {
                const others = libraryAccess.libraryIds.filter((each) => each !== id);
                choose(checked ? [...others, id] : others);
              }}
            />
          ))}
        </fieldset>
      )}
    </>
  );
}
