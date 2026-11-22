import { BoxSetArt } from "@/features/title/components/box-set-art";
import { Overview } from "@/features/title/components/overview";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";

/** A Box Set page's top: its Backdrop, as a title's, with its Poster, name and overview. */
export function BoxSetTop({ boxSet }: { boxSet: BoxSet }) {
  return (
    <TitleHero backdrop={{ url: boxSet.backdropUrl, thumbhash: undefined }}>
      {boxSet.posterUrl !== undefined && <BoxSetArt url={boxSet.posterUrl} />}
      <TitleName title={boxSet.name} logoUrl={undefined} />
      {boxSet.overview !== undefined && <Overview text={boxSet.overview} />}
    </TitleHero>
  );
}
