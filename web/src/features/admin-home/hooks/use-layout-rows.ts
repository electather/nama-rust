import { useState } from "react";

import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { moved, moveAnnouncement } from "@/shared/lib/home-layout";

/**
 * The rows of a Home layout as an Admin changes them, from `saved`: moving one, which says where
 * it went for assistive technology to announce, and showing or hiding one.
 */
export function useLayoutRows(saved: HomeLayoutRow[], names: Map<string, string>) {
  const [rows, setRows] = useState(saved);
  const [announced, setAnnounced] = useState("");

  const move = (from: number, before: number) => {
    const row = rows[from];
    const next = moved(rows, from, before);
    setRows(next);
    if (row !== undefined && next.indexOf(row) !== from) {
      setAnnounced(moveAnnouncement(row, next, names));
    }
  };
  const setShown = (index: number, shown: boolean) => {
    setRows(rows.map((row, each) => (each === index ? withHidden(row, !shown) : row)));
  };
  return { rows, move, setShown, announced };
}

function withHidden(row: HomeLayoutRow, hidden: boolean): HomeLayoutRow {
  return { ...row, hidden };
}
