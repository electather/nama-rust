import { useState } from "react";

import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { moved, moveAnnouncement } from "@/shared/lib/home-layout";

/**
 * Moving a row of `rows`, which hands `save` the new order and says where the row went for
 * assistive technology to announce. A drop in place saves nothing.
 */
export function useRowMoves(
  rows: HomeLayoutRow[],
  save: (rows: HomeLayoutRow[]) => void,
  names?: Map<string, string>,
) {
  const [announced, setAnnounced] = useState("");
  const move = (from: number, before: number) => {
    const row = rows[from];
    const next = moved(rows, from, before);
    if (row !== undefined && next.indexOf(row) !== from) {
      save(next);
      setAnnounced(moveAnnouncement(row, next, names));
    }
  };
  return { move, announced };
}
