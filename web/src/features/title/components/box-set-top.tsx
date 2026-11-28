import { Overview } from "@/features/title/components/overview";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import { PosterArt } from "@/shared/components/poster/poster-art";
import { backdropOf, posterOf } from "@/shared/lib/server-image";

/** A Box Set page's top: its Backdrop, as a title's, with its Poster, name and overview. */
export function BoxSetTop({ boxSet }: { boxSet: BoxSet }) {
  const poster = posterOf(boxSet);
  return (
    <TitleHero backdrop={backdropOf(boxSet)}>
      {poster && (
        <div className="w-32 sm:w-40">
          <PosterArt art={poster} title={boxSet.name} size="shelf" />
        </div>
      )}
      <TitleName title={boxSet.name} logo={undefined} />
      {boxSet.overview !== undefined && <Overview text={boxSet.overview} />}
    </TitleHero>
  );
}
