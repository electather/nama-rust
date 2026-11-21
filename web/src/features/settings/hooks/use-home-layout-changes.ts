import { create } from "@bufbuild/protobuf";
import { useState } from "react";

import { useReorderHomeRows } from "@/features/settings/data/use-reorder-home-rows";
import { useSetHomeLayout } from "@/features/settings/data/use-set-home-layout";
import { type HomeLayout, type HomeLayoutRow, HomeLayoutSchema } from "@/gen/nama/v1/server_pb";
import { ReorderHomeRowsRequestSchema } from "@/gen/nama/v1/user_pb";
import { moved, moveAnnouncement, rowKey } from "@/shared/lib/home-layout";

/**
 * The signed-in User's Home layout as they change it from `saved`, each change saved as it's made
 * and shown until the layout comes back with it: showing or hiding Featured or a row, and moving
 * a row, which says where it went for assistive technology to announce.
 */
export function useHomeLayoutChanges(saved: HomeLayout) {
  const save = useSetHomeLayout();
  const reorder = useReorderHomeRows();
  const [announced, setAnnounced] = useState("");
  const ordered = reorder.isPending
    ? inOrder(saved, create(ReorderHomeRowsRequestSchema, reorder.variables).rows)
    : saved;
  const layout = save.isPending
    ? saving(ordered, create(HomeLayoutSchema, save.variables.layout))
    : ordered;

  const setFeaturedShown = (shown: boolean) => {
    save.mutate({ layout: { featuredHidden: !shown } });
  };
  // Names only the flipped row, and leaves Featured unset: what the User leaves alone then keeps
  // following the default, even where it changed since this list was loaded
  // (`UserService.SetHomeLayout`).
  const setShown = (row: HomeLayoutRow, shown: boolean) => {
    save.mutate({ layout: { rows: [{ ...row, hidden: !shown }] } });
  };
  // A drop in place saves nothing, since the User's first order stops the default's moving theirs.
  const move = (from: number, before: number) => {
    const row = layout.rows[from];
    const rows = moved(layout.rows, from, before);
    if (row !== undefined && rows.indexOf(row) !== from) {
      reorder.mutate({ rows });
      setAnnounced(moveAnnouncement(row, rows));
    }
  };
  return {
    layout,
    setFeaturedShown,
    setShown,
    move,
    announced,
    error: save.error ?? reorder.error,
  };
}

/**
 * `saved` with its rows in the order of `order`, each shown as `saved` has it, since a switch
 * flipped after the move may have saved since.
 */
function inOrder(saved: HomeLayout, order: HomeLayoutRow[]): HomeLayout {
  const rows = new Map(saved.rows.map((row) => [rowKey(row), row]));
  return { ...saved, rows: order.map((row) => rows.get(rowKey(row)) ?? row) };
}

/** `saved` with what `asked` changes in it. */
function saving(saved: HomeLayout, asked: HomeLayout): HomeLayout {
  const named = new Map(asked.rows.map((row) => [rowKey(row), row]));
  return {
    ...saved,
    featuredHidden: asked.featuredHidden ?? saved.featuredHidden,
    rows: saved.rows.map((row) => named.get(rowKey(row)) ?? row),
  };
}
