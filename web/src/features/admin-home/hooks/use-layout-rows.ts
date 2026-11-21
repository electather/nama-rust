import { useState } from "react";

import { moved } from "@/features/admin-home/lib/home-layout";
import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { rowName } from "@/shared/lib/home-layout";

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
      const { label } = rowName(row, names);
      setAnnounced(`${label} moved to place ${next.indexOf(row) + 1} of ${next.length}.`);
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
