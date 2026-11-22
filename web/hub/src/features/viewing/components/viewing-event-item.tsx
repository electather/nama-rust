import { useDeleteViewingEvent } from "@hub/features/viewing/data/viewing-events";
import {
  playedHowFar,
  playedTitle,
  playedWhen,
} from "@hub/features/viewing/lib/viewing-event-text";
import type { ViewingEvent } from "@hub/gen/nama/hub/v1/viewing_pb";
import { Button } from "nama-ui/components/button";

/** One Viewing Event: what played, when and on what, how far it got, and deleting it. */
export function ViewingEventItem({ event }: { event: ViewingEvent }) {
  const remove = useDeleteViewingEvent();
  const title = playedTitle(event);
  const when = playedWhen(event);
  return (
    <li className="flex items-center justify-between gap-3 py-2.5">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-base break-all">{title}</span>
        <span className="text-xs text-muted-foreground">{when}</span>
        <span className="text-xs text-muted-foreground">{playedHowFar(event)}</span>
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {remove.error ? "The Hub couldn’t delete this just now. Try again." : undefined}
        </p>
      </div>
      <Button
        variant="ghost"
        aria-label={`Delete ${title}, ${when}`}
        disabled={remove.isPending}
        onClick={() => {
          remove.mutate({ id: event.id });
        }}
      >
        {remove.isPending ? "Deleting…" : "Delete"}
      </Button>
    </li>
  );
}
