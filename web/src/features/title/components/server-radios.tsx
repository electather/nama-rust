import { DropdownMenuRadioGroup, DropdownMenuRadioItem } from "nama-ui/components/dropdown-menu";

import type { PlayFrom } from "@/features/title/lib/copies";
import { metadataLine } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";
import { versionToPlay } from "@/shared/lib/version-to-play";

/** Each Server of `copies` by its name and the Version Play would play there, `copy`'s checked. */
export function ServerRadios({ copies, copy, onChoose }: PlayFrom) {
  return (
    <DropdownMenuRadioGroup
      value={copy?.server?.id}
      onValueChange={(value) => {
        onChoose(String(value));
      }}
    >
      {copies.map(({ server, versions }) => (
        <DropdownMenuRadioItem key={server?.id} value={server?.id}>
          {metadataLine(server?.name, versionLine(versionToPlay(versions)))}
        </DropdownMenuRadioItem>
      ))}
    </DropdownMenuRadioGroup>
  );
}
