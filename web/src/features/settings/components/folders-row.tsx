import { Textarea } from "nama-ui/components/textarea";

import {
  foldersHint,
  foldersTyped,
  foldersWith,
} from "@/features/admin-libraries/lib/library-form";
import { FolderPicker } from "@/shared/components/folder-picker";
import { SettingRow } from "@/shared/components/setting-row";

/**
 * A Library's folders, one per line, saved once the field is left, never while they're typed:
 * half a path would be scanned as a folder of its own. A folder picked beside it is saved as a new
 * line at once, unless it's listed already.
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
        <div className="flex w-full items-start gap-2">
          <Textarea
            {...control}
            // Uncontrolled, so a refused save that puts `saved` back must remount it to show that.
            key={saved.join("\n")}
            spellCheck={false}
            defaultValue={saved.join("\n")}
            onBlur={(event) => {
              const folders = foldersTyped(event.currentTarget.value);
              if (folders.join("\n") !== saved.join("\n")) {
                onSave(folders);
              }
            }}
          />
          <FolderPicker
            path={saved.at(-1) ?? ""}
            onPick={(picked) => {
              const folders = foldersWith(saved, picked);
              if (folders !== saved) {
                onSave(folders);
              }
            }}
          />
        </div>
      )}
    </SettingRow>
  );
}
