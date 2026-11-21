import { useState } from "react";

import { serverLabels, urlProblem } from "@/features/settings/lib/server-values";
import { SettingRow } from "@/shared/components/setting-row";
import { Input } from "@/shared/ui/input";

/**
 * The Server URL on its row, saved once the field is left or Enter is pressed in it, never while
 * it's typed: half an address would be the Server's address for everyone. What's wrong with it
 * shows then too, so it isn't called wrong while it's still being typed.
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
  const [typed, setTyped] = useState(saved);
  // The address last sent to be saved, so leaving the field again doesn't send it twice.
  const [sent, setSent] = useState(saved);
  const [left, setLeft] = useState(false);
  const problem = urlProblem(typed);
  const shown = left ? (problem ?? refused) : undefined;

  /** Saves what's typed, unless it can't be an address or is `already` the one to save. */
  function commit(already: string) {
    const url = typed.trim();
    setLeft(true);
    if (problem === undefined && url !== already) {
      setSent(url);
      onSave(url);
    }
  }

  return (
    <SettingRow
      label={serverLabels.url}
      hint="The address people reach this Server at. Links in messages and Invites start with it. Leave it empty for no links, and no Invites. It’s saved for everyone when you leave the field."
      error={shown}
    >
      {(control) => (
        <span className="flex w-40 md:w-60">
          <Input
            {...control}
            type="url"
            inputMode="url"
            autoComplete="url"
            placeholder="https://nama.example.com"
            aria-invalid={shown !== undefined}
            value={typed}
            onChange={(event) => {
              setTyped(event.target.value);
            }}
            onBlur={() => {
              commit(sent);
            }}
            onKeyDown={(event) => {
              // Enter sends whatever the Server doesn't have, so it also tries a refused one again.
              if (event.key === "Enter") {
                commit(saved);
              }
            }}
          />
        </span>
      )}
    </SettingRow>
  );
}
