import type { Version } from "@/gen/nama/v1/browse_pb";
import { metadataLine, sizeOf } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";

/** A choice of Version, each with its file, picture, sound and size. */
export function VersionChoice({
  versions,
  versionId,
  onChoose,
}: {
  versions: Version[];
  versionId: string;
  onChoose: (versionId: number) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm text-muted-foreground">
      Version
      <select
        value={versionId}
        onChange={(event) => {
          onChoose(Number(event.target.value));
        }}
        className="h-8 max-w-64 truncate rounded-lg border border-input bg-input/30 px-2.5 text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {versions.map((option) => (
          <option key={String(option.id)} value={String(option.id)}>
            {option.fileName} ({metadataLine(versionLine(option), sizeOf(option.size))})
          </option>
        ))}
      </select>
    </label>
  );
}
