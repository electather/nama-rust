import { Textarea } from "nama-ui/components/textarea";

import { foldersHint, foldersTyped } from "@/features/admin-libraries/lib/library-form";
import { SettingRow } from "@/shared/components/setting-row";

/**
 * A Library's folders, one per line, saved once the field is left, never while they're typed:
 * half a path would be scanned as a folder of its own.
 */
export function FoldersRow({
  saved,
  onSave,
}: {
  saved: string[];
  onSave: (folders: string[]) => void;
}) {
  return (
    <SettingRow wide label="Folders" hint={foldersHint} size="field">
      {(control) => (
        <Textarea
          {...control}
          spellCheck={false}
          defaultValue={saved.join("\n")}
          onBlur={(event) => {
            const folders = foldersTyped(event.currentTarget.value);
            if (folders.join("\n") !== saved.join("\n")) {
              onSave(folders);
            }
          }}
        />
      )}
    </SettingRow>
  );
}
