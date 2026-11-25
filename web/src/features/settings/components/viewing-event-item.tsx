import { Film, Tv } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { ItemRow } from "@/features/settings/components/item-row";
import { useDeleteViewingEvent } from "@/features/settings/data/use-delete-viewing-event";
import { playedHowFar, playedTitle, playedWhen } from "@/features/settings/lib/viewing-event-text";
import type { ViewingEvent } from "@/gen/nama/hub/v1/viewing_pb";
import { IconTile } from "@/shared/components/icon-tile";

/** One Viewing Event: what played, when and on what, how far it got, and deleting it. */
export function ViewingEventItem({ event }: { event: ViewingEvent }) {
  const remove = useDeleteViewingEvent();
  const title = playedTitle(event);
  const when = playedWhen(event);
  return (
    <ItemRow
      beside
      start={<IconTile Icon={event.episode ? Tv : Film} />}
      name={title}
      meta={`${when} · ${playedHowFar(event)}`}
      failed={remove.error ? "The Hub couldn’t delete it just now. Try again." : undefined}
    >
      <Button
        variant="ghost"
        aria-label={`Delete ${title}, ${when}`}
        disabled={remove.isPending}
        onClick={() => {
          remove.mutate({ id: event.id });
        }}
      >
        Delete
      </Button>
    </ItemRow>
  );
}
