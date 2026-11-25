import { toJsonString } from "@bufbuild/protobuf";
import { Button } from "nama-ui/components/button";

import {
  type ListViewingEventsResponse,
  ListViewingEventsResponseSchema,
} from "@/gen/nama/hub/v1/viewing_pb";

/** Downloads `history`, every Viewing Event, as one JSON file in the Hub API's own form. */
export function ExportHistoryButton({ history }: { history: ListViewingEventsResponse }) {
  return (
    <Button
      variant="secondary"
      onClick={() => {
        const json = toJsonString(ListViewingEventsResponseSchema, history, { prettySpaces: 2 });
        // A Blob, not a data: URL, which a long history could make too long for the browser.
        const file = URL.createObjectURL(new Blob([json], { type: "application/json" }));
        const download = document.createElement("a");
        download.href = file;
        download.download = "nama-viewing-history.json";
        download.click();
        // Once the download has started with it.
        setTimeout(() => {
          URL.revokeObjectURL(file);
        }, 0);
      }}
    >
      Export as JSON
    </Button>
  );
}
