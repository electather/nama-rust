import { ViewingEventItem } from "@hub/features/viewing/components/viewing-event-item";
import { useViewingEvents } from "@hub/features/viewing/data/viewing-events";
import { exportedHistory } from "@hub/features/viewing/lib/viewing-event-text";
import { buttonVariants } from "nama-ui/components/button";

/** The signed-in Account's Viewing Events, exporting them, or what stands in for them. */
export function ViewingHistory() {
  const events = useViewingEvents();
  if (events.isPending) {
    return <p className="text-sm text-muted-foreground">Looking up your history…</p>;
  }
  if (events.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list your history just now. Reload the page to try again.
      </p>
    );
  }
  if (events.data.events.length === 0) {
    return <p className="text-sm text-muted-foreground">Nothing played yet.</p>;
  }
  return (
    <>
      <a
        href={exportedHistory(events.data)}
        download="nama-viewing-history.json"
        className={buttonVariants({ variant: "outline", size: "lg" })}
      >
        Export as JSON
      </a>
      <ul className="flex flex-col divide-y">
        {events.data.events.map((event) => (
          <ViewingEventItem key={event.id} event={event} />
        ))}
      </ul>
    </>
  );
}
