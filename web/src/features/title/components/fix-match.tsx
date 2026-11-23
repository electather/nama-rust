import { useNavigate } from "@tanstack/react-router";
import { type Ref, useImperativeHandle, useRef, useState } from "react";

import { Unmatch } from "@/features/title/components/unmatch";
import { type ExternalId, type SetMatchResponse, TitleKind } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { MatchDialog } from "@/shared/components/find-a-match/match-dialog";
import { useMay } from "@/shared/hooks/use-may";
import type { TitleToMatch } from "@/shared/lib/find-a-match";
import type { Opener } from "@/shared/lib/opener";
import { titleParam } from "@/shared/lib/title-param";

/**
 * With Manage Libraries, the dialogs of a Matched Movie's or Show's Fix Match, which Matches its
 * every Item to the title picked, pinned, and goes to that title, and of its Unmatch. Only `fix`
 * and `unmatch` open them, from the "…" menu.
 */
export function FixMatch({
  fix,
  unmatch,
  title,
}: {
  fix: Ref<Opener>;
  unmatch: Ref<Opener>;
  title: TitleToMatch;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  // How many times it has opened, so each opening searches afresh for what the page shows now.
  const [opened, setOpened] = useState(0);
  const navigate = useNavigate();
  useImperativeHandle(fix, () => ({
    open: () => {
      dialog.current?.showModal();
      setOpened(opened + 1);
    },
  }));
  if (!useMay(Permission.MANAGE_LIBRARIES)) {
    return null;
  }
  const libraryId = String(title.matchedTitle.libraryId);
  const goToMatched = (titleId: ExternalId | undefined, { matched }: SetMatchResponse) => {
    dialog.current?.close();
    if (matched === 0n) {
      // A Show that has none of its Episodes: they're all on Needs a Match.
      void navigate({ to: "/libraries/$libraryId", params: { libraryId } });
    } else if (title.kind === TitleKind.MOVIE) {
      const movieId = titleParam(titleId);
      void navigate({
        to: "/libraries/$libraryId/movies/$movieId",
        params: { libraryId, movieId },
      });
    } else {
      const showId = titleParam(titleId);
      void navigate({ to: "/libraries/$libraryId/shows/$showId", params: { libraryId, showId } });
    }
  };
  return (
    <>
      <MatchDialog
        dialog={dialog}
        opened={opened}
        item={title}
        libraryId={title.matchedTitle.libraryId}
        onMatched={goToMatched}
      />
      <Unmatch ref={unmatch} title={title} />
    </>
  );
}
