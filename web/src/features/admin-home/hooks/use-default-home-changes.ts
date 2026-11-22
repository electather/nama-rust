import { useState } from "react";

import { useDefaultHomeLayout } from "@/features/admin-home/data/use-default-home-layout";
import { useSetDefaultHomeLayout } from "@/features/admin-home/data/use-set-default-home-layout";
import type { HomeLayout, HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { useRowMoves } from "@/shared/hooks/use-row-moves";

/**
 * The default Home layout as an Admin changes it, each change saved as it's made and shown until
 * the layout comes back with it, or until it's refused: showing or hiding Featured or a row, and
 * moving a row, which says where it went for assistive technology to announce.
 */
export function useDefaultHomeChanges(names: Map<string, string>) {
  const saved = useDefaultHomeLayout();
  const save = useSetDefaultHomeLayout();
  // The newest layout asked for, until its save settles: each holds every change before it, as
  // each sends the whole layout. The mutation's own variables reach a render too late for a quick
  // next change to build on.
  const [asked, setAsked] = useState<HomeLayout>();
  const layout = asked ?? saved;

  const saveWith = (changes: Partial<HomeLayout>) => {
    const next = { ...layout, ...changes };
    setAsked(next);
    save.mutate(
      { layout: next },
      {
        onSettled: () => {
          setAsked((now) => (now === next ? undefined : now));
        },
      },
    );
  };
  const setFeaturedShown = (shown: boolean) => {
    saveWith({ featuredHidden: !shown });
  };
  const setShown = (changed: HomeLayoutRow, shown: boolean) => {
    saveWith({
      rows: layout.rows.map((row) => (row === changed ? { ...row, hidden: !shown } : row)),
    });
  };
  const { move, announced } = useRowMoves(
    layout.rows,
    (rows) => {
      saveWith({ rows });
    },
    names,
  );
  return { layout, setFeaturedShown, setShown, move, announced, error: save.error };
}
