import { ExportHistoryButton } from "@/features/settings/components/export-history-button";
import { ViewingEventItem } from "@/features/settings/components/viewing-event-item";
import { useViewingEvents } from "@/features/settings/data/use-viewing-events";

/** The signed-in Account's Viewing Events, each with Delete, and exporting them all. */
export function ViewingEventItems() {
  const history = useViewingEvents();
  if (history.events.length === 0) {
    return <p className="py-2.5 text-sm text-muted-foreground">Nothing played yet.</p>;
  }
  return (
    <>
      <ul aria-label="Viewing history" className="-mb-1.5 flex flex-col">
        {history.events.map((event) => (
          <ViewingEventItem key={event.id} event={event} />
        ))}
      </ul>
      <div className="flex justify-end pt-2.5">
        <ExportHistoryButton history={history} />
      </div>
    </>
  );
}
