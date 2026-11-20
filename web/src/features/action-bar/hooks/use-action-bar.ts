import { useNavigate } from "@tanstack/react-router";
import { type KeyboardEvent, useState } from "react";

import { useFound } from "@/features/action-bar/data/use-found";
import { useDebounced } from "@/features/action-bar/hooks/use-debounced";
import { useListed } from "@/features/action-bar/hooks/use-listed";
import { useRecents } from "@/features/action-bar/hooks/use-recents";
import { groupsOf } from "@/features/action-bar/lib/groups";
import { press } from "@/features/action-bar/lib/keys";
import { type Page, pagesFor } from "@/features/action-bar/lib/pages";
import type { Frame, Opening, Row } from "@/features/action-bar/lib/rows";
import type { FoundTitle } from "@/features/action-bar/lib/titles";
import { TitleKind } from "@/gen/nama/v1/match_pb";

/** What a search from each frame asks the Server for. */
const kinds = {
  root: TitleKind.UNSPECIFIED,
  movie: TitleKind.MOVIE,
  show: TitleKind.SHOW,
  shortcuts: TitleKind.UNSPECIFIED,
} satisfies Record<Frame, TitleKind>;

/** The row selected: the one picked, while it's still listed, or else the first. */
function activeOf(rows: readonly Row[], picked: string | undefined) {
  return rows.find((row) => row.id === picked) ?? rows[0];
}

/** The open action bar: where it is, what's typed, what it lists and which row is selected. */
export function useActionBar({
  opening,
  hasAdminPages,
  dialog,
}: {
  opening: Opening;
  hasAdminPages: boolean;
  /** What the dialog can be asked to do: close, and put focus back in its field. */
  dialog: { close: () => void; focusField: () => void };
}) {
  const [frame, setFrame] = useState(opening.frame);
  const [query, setQuery] = useState(opening.query);
  const [picked, setPicked] = useState<string>();
  const navigate = useNavigate();
  const { recents, remember } = useRecents();
  const typed = frame === "shortcuts" ? "" : query.trim();
  const words = useDebounced(typed, 300);
  const search = useFound(words, kinds[frame]);
  const { listed, slow, clear } = useListed(typed, words, search.searching);

  function push(next: Frame) {
    setFrame(next);
    setQuery("");
    clear();
    setPicked(undefined);
    // Picking a row or the filter's chip with the pointer mustn't end the typing.
    dialog.focusField();
  }
  function back() {
    push("root");
  }
  function go(to: Page["to"]) {
    dialog.close();
    void navigate({ to });
  }
  function open(title: FoundTitle) {
    remember(title);
    dialog.close();
    const { libraryId, titleId, season } = title;
    void (title.kind === "movie"
      ? navigate({
          to: "/libraries/$libraryId/movies/$movieId",
          params: { libraryId, movieId: titleId },
        })
      : navigate({
          to: "/libraries/$libraryId/shows/$showId",
          params: { libraryId, showId: titleId },
          search: season === undefined ? {} : { season },
        }));
  }

  const groups = groupsOf(
    {
      frame,
      // Shortcuts filter as they are typed: there is nothing to wait for.
      query: frame === "shortcuts" ? query : listed,
      pages: pagesFor(hasAdminPages),
      recents,
      found: search.found,
      failed: search.failed,
    },
    { push, go, open, retry: search.retry },
  );
  const rows = groups.flatMap((group) => group.rows);
  const active = activeOf(rows, picked);
  function step(by: number) {
    const at = active === undefined ? 0 : rows.indexOf(active);
    const next = rows[(at + by + rows.length) % rows.length];
    setPicked(next?.id);
  }

  return {
    frame,
    query,
    listed,
    groups,
    active,
    slow,
    handleClose: dialog.close,
    handleBack: back,
    pick: setPicked,
    type: (next: string) => {
      setQuery(next);
      setPicked(undefined);
    },
    /** The field's keys: moving through the rows, picking one, and stepping back out of a frame. */
    handleFieldKeyDown: (event: KeyboardEvent) => {
      // Enter that ends an IME composition picks a word, not a row.
      if (event.nativeEvent.isComposing) {
        return;
      }
      press(event, {
        ArrowDown: () => {
          step(1);
        },
        ArrowUp: () => {
          step(-1);
        },
        Enter: () => active?.run?.(),
        Backspace: query === "" && frame !== "root" ? back : undefined,
      });
    },
    /**
     * Keys that work wherever focus is in the dialog. At the root Escape is left to the dialog,
     * which closes on it.
     */
    handleKeyDown: (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;
      press(
        event,
        mod
          ? {
              "/": () => {
                push("shortcuts");
              },
            }
          : { Escape: frame === "root" ? undefined : back },
      );
    },
  };
}

/** What `useActionBar` gives the action bar's parts. */
export type Bar = ReturnType<typeof useActionBar>;
