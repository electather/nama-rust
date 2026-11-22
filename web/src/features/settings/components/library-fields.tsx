import { RecognisersField } from "@/features/admin-libraries/components/recognisers-field/recognisers-field";
import { TitleSourcesField } from "@/features/admin-libraries/components/title-sources-field/title-sources-field";
import { RECOGNISERS, SEGMENT_SOURCES } from "@/features/admin-libraries/lib/recognisers";
import { FoldersRow } from "@/features/settings/components/folders-row";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { LibraryValues } from "@/features/settings/hooks/use-library-changes";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";

/**
 * A Library's name and folders, saved once their field is left, then its title sources,
 * Recognisers and Segment sources, saved as their order changes.
 */
export function LibraryFields({
  values,
  onEdit,
}: {
  values: LibraryValues;
  onEdit: (changes: Partial<LibraryValues>) => void;
}) {
  return (
    <>
      <SettingGroup title="Name and folders">
        <TypedSettingRow
          label="Name"
          hint={undefined}
          saved={values.name}
          error={undefined}
          input={{ autoComplete: "off" }}
          onSave={(name) => {
            onEdit({ name });
          }}
        />
        <FoldersRow
          saved={values.folders}
          onSave={(folders) => {
            onEdit({ folders });
          }}
        />
      </SettingGroup>
      <TitleSourcesField
        order={values.titleSources}
        setOrder={(titleSources) => {
          onEdit({ titleSources });
        }}
      />
      <RecognisersField
        kind={RECOGNISERS}
        order={values.recognisers}
        setOrder={(recognisers) => {
          onEdit({ recognisers });
        }}
      />
      <RecognisersField
        kind={SEGMENT_SOURCES}
        order={values.segmentSources}
        setOrder={(segmentSources) => {
          onEdit({ segmentSources });
        }}
      />
    </>
  );
}
