import type { Version } from "@/gen/nama/v1/browse_pb";
import { metadataLine, sizeOf } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";
import { DropdownMenuRadioGroup, DropdownMenuRadioItem } from "@/shared/ui/dropdown-menu";

/** Each of `versions` by its Edition, picture, sound and size, `version` checked. */
export function VersionRadios({
  versions,
  version,
  onChoose,
}: {
  versions: Version[];
  version: Version;
  onChoose: (versionId: number) => void;
}) {
  return (
    <DropdownMenuRadioGroup
      value={String(version.id)}
      onValueChange={(value) => {
        onChoose(Number(value));
      }}
    >
      {versions.map((option) => (
        <DropdownMenuRadioItem key={String(option.id)} value={String(option.id)}>
          {metadataLine(option.edition, versionLine(option), sizeOf(option.size))}
        </DropdownMenuRadioItem>
      ))}
    </DropdownMenuRadioGroup>
  );
}
