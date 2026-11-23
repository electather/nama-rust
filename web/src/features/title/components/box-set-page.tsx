import { notFound } from "@tanstack/react-router";

import { BoxSetMovies } from "@/features/title/components/box-set-movies";
import { BoxSetTop } from "@/features/title/components/box-set-top";
import { TitlePage } from "@/features/title/components/title-page";
import { useBoxSet } from "@/features/title/data/use-box-set";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Box Set: its Backdrop, Poster, name and overview at the top, then its Movies in release order,
 * those missing from the Server opening their page to Request.
 */
export function BoxSetPage({ boxSetId }: { boxSetId: ExternalId }) {
  const { boxSet } = useBoxSet({ boxSetId });
  if (!boxSet) {
    throw notFound();
  }
  return (
    <TitlePage name={boxSet.name} hero={<BoxSetTop boxSet={boxSet} />}>
      <BoxSetMovies boxSet={boxSet} />
    </TitlePage>
  );
}
