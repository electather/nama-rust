import { notFound } from "@tanstack/react-router";

import { SeasonPicker } from "@/features/title/components/season-picker";
import { TitlePage } from "@/features/title/components/title-page";
import { useFoundShow } from "@/features/title/data/use-found-show";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/** A Show from the title source, for the signed-in User `you` to pick its Seasons to Request. */
export function ShowToRequest({ showId, you }: { showId: ExternalId; you: bigint | undefined }) {
  const show = useFoundShow({ showId });
  if (!show) {
    throw notFound();
  }
  return (
    <TitlePage
      title={show.title}
      backdropUrl={show.backdropUrl}
      backdropThumbhash={show.backdropThumbhash}
    >
      <p className="text-sm text-muted-foreground">{show.year}</p>
      {show.overview !== undefined && (
        <p className="mt-6 max-w-prose text-pretty">{show.overview}</p>
      )}
      <SeasonPicker show={show} you={you} />
    </TitlePage>
  );
}
