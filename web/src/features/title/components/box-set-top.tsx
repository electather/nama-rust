import { BoxSetArt } from "@/features/title/components/box-set-art";
import { Overview } from "@/features/title/components/overview";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import { backdropOf, posterOf } from "@/shared/lib/server-image";

/** A Box Set page's top: its Backdrop, as a title's, with its Poster, name and overview. */
export function BoxSetTop({ boxSet }: { boxSet: BoxSet }) {
  const poster = posterOf(boxSet);
  return (
    <TitleHero backdrop={backdropOf(boxSet)}>
      {poster && <BoxSetArt poster={poster} />}
      <TitleName title={boxSet.name} logo={undefined} />
      {boxSet.overview !== undefined && <Overview text={boxSet.overview} />}
    </TitleHero>
  );
}
