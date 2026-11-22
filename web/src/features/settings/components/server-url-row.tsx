import { serverLabels, urlProblem } from "@/features/settings/lib/server-values";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";
import { rowText } from "@/shared/lib/setting-text";

/**
 * The Server URL on its row, saved once the field is left, never while it's typed: half an address
 * would be the Server's address for everyone.
 */
export function ServerUrlRow({
  saved,
  refused,
  onSave,
}: {
  saved: string;
  /** Why the Server refused the address last sent, if it did. */
  refused: string | undefined;
  onSave: (url: string) => void;
}) {
  return (
    <TypedSettingRow
      {...rowText(serverLabels, "url")}
      saved={saved}
      error={refused}
      check={urlProblem}
      input={{
        type: "url",
        inputMode: "url",
        autoComplete: "url",
        placeholder: "https://nama.example.com",
      }}
      onSave={onSave}
    />
  );
}
