import { DropdownMenuRadioGroup, DropdownMenuRadioItem } from "nama-ui/components/dropdown-menu";

import type { PlayFrom } from "@/features/title/lib/server-choices";
import { metadataLine } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";
import { versionToPlay } from "@/shared/lib/version-to-play";

/** Each Server of `choices` by its name and the Version Play would play there, `choice`'s checked. */
export function ServerRadios({ choices, choice, onChoose }: PlayFrom) {
  return (
    <DropdownMenuRadioGroup
      value={choice?.server?.id}
      onValueChange={(value) => {
        onChoose(String(value));
      }}
    >
      {choices.map(({ server, versions }) => (
        <DropdownMenuRadioItem key={server?.id} value={server?.id}>
          {metadataLine(server?.name, versionLine(versionToPlay(versions)))}
        </DropdownMenuRadioItem>
      ))}
    </DropdownMenuRadioGroup>
  );
}
