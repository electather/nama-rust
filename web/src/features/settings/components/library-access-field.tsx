import { create } from "@bufbuild/protobuf";

import { SwitchRow } from "@/features/settings/components/switch-row";
import { needsEveryLibrary } from "@/features/settings/lib/needs-every-library";
import { type LibraryAccess, LibraryAccessSchema, type Permission } from "@/gen/nama/v1/sign_in_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { useLibraries } from "@/shared/hooks/use-libraries";

const ALL = "all";
const CHOSEN = "chosen";

/**
 * The Libraries a User holding `permissions` sees, `libraryAccess`, unset for every one, as a row
 * and, for a chosen list, a switch for each Library, each change going to `onChange`, to save at
 * once. Chosen Libraries isn't offered to a User who must see every Library.
 */
export function LibraryAccessField({
  libraryAccess,
  permissions,
  onChange,
}: {
  libraryAccess: LibraryAccess | undefined;
  permissions: Permission[];
  onChange: (changes: { libraryAccess: LibraryAccess | undefined }) => void;
}) {
  // Every Library: whoever may edit a User holds Manage Users, which the Server gives only to a User
  // who sees every Library.
  const libraries = useLibraries();
  const everyOne = needsEveryLibrary(permissions);
  const choose = (libraryIds: bigint[]) => {
    onChange({ libraryAccess: create(LibraryAccessSchema, { libraryIds }) });
  };
  return (
    <>
      <FormSelect
        row
        label="Libraries"
        hint={
          everyOne
            ? "A User with Manage Users sees every Library."
            : "All Libraries takes in those added later. A User with Manage Libraries sees every one."
        }
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
          { value: CHOSEN, label: "Chosen Libraries", disabled: everyOne },
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
