import type { MessageInitShape } from "@bufbuild/protobuf";
import { notFound } from "@tanstack/react-router";

import { Player } from "@/features/player/components/player";
import { ProgressNotSaved } from "@/features/player/components/progress-not-saved";
import { useUnmatchedVersion } from "@/features/player/data/use-unmatched-version";
import type { GetUnmatchedVersionRequestSchema } from "@/gen/nama/v1/browse_pb";
import { isGone } from "@/shared/hooks/browse";
import { parsedAs } from "@/shared/lib/unmatched-names";

/** Plays an Unmatched Item's Version, which keeps no Watch State (ADR 0013). */
export function UnmatchedPlayer({
  input,
  libraryId,
}: {
  input: MessageInitShape<typeof GetUnmatchedVersionRequestSchema> & { libraryId: bigint };
  libraryId: string;
}) {
  // A refetch that finds it Matched since keeps what was read, so playing goes on.
  const { data, error } = useUnmatchedVersion(input);
  const { item, version } = data;
  if (!item || !version) {
    throw notFound();
  }
  // Gone since means Matched, so its progress is saved, or its Version removed, which plays no more.
  const matched = isGone(error);
  return (
    <Player
      title={parsedAs(item)}
      versionId={version.id}
      tracks={version.tracks}
      watchState={undefined}
      back={{ to: "/libraries/$libraryId", params: { libraryId } }}
      note={!matched && <ProgressNotSaved item={item} libraryId={input.libraryId} />}
    />
  );
}
