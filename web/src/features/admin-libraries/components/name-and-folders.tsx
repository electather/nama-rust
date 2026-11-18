import { KindChoice } from "@/features/admin-libraries/components/kind-choice";
import { TitleSourcesField } from "@/features/admin-libraries/components/title-sources-field/title-sources-field";
import type { Library } from "@/gen/nama/v1/library_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { FormTextarea } from "@/shared/components/form-field/form-textarea";
import type { FormErrors } from "@/shared/lib/forms";

/** The name, folders and title sources fields, and for a new Library what it holds. */
export function NameAndFolders({ errors, library }: { errors: FormErrors; library?: Library }) {
  return (
    <>
      <FormField
        label="Name"
        name="name"
        autoComplete="off"
        defaultValue={library?.name}
        error={errors.name}
      />
      <KindChoice library={library} />
      <FormTextarea
        label="Folders"
        name="folders"
        spellCheck={false}
        defaultValue={library?.folders.join("\n")}
        hint="Where its files are on the Server: one full path per line, such as /srv/media/films."
        error={errors.folders}
      />
      <TitleSourcesField initial={library?.titleSources} error={errors.titleSources} />
    </>
  );
}
