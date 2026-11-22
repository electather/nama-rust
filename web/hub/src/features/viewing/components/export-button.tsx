import { exportedHistory } from "@hub/features/viewing/lib/viewing-event-text";
import type { ListViewingEventsResponse } from "@hub/gen/nama/hub/v1/viewing_pb";
import { Button } from "nama-ui/components/button";

/** Downloads every Viewing Event as one JSON file. */
export function ExportButton({ history }: { history: ListViewingEventsResponse }) {
  return (
    <Button
      variant="outline"
      size="lg"
      onClick={() => {
        // A Blob, not a data: URL, which a long history could make too long for the browser.
        const url = URL.createObjectURL(exportedHistory(history));
        const link = document.createElement("a");
        link.href = url;
        link.download = "nama-viewing-history.json";
        link.click();
        // Once the download has started with it.
        setTimeout(() => {
          URL.revokeObjectURL(url);
        }, 0);
      }}
    >
      Export as JSON
    </Button>
  );
}
