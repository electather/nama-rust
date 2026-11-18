import { useNavigate } from "@tanstack/react-router";
import { Suspense, lazy, useCallback, useMemo, useState } from "react";

import { useActionBarKeys } from "@/features/action-bar/hooks/use-action-bar-keys";
import type { FoundTitle } from "@/features/action-bar/lib/found-titles";
import type { Opening } from "@/features/action-bar/lib/menu";
import { pagesFor } from "@/features/action-bar/lib/pages";

const ActionBarDialog = lazy(async () => {
  const { ActionBarDialog: dialog } =
    await import("@/features/action-bar/components/action-bar-dialog");
  return { default: dialog };
});

/**
 * The action bar, the one search, open while `opening` is set. Its shortcuts open it from
 * anywhere unless `shortcuts` is off. It remembers the last titles opened from it while the User
 * stays signed in.
 */
export function ActionBar({
  opening,
  hasAdminPages,
  shortcuts,
  onOpen,
  onClose,
}: {
  opening: Opening | undefined;
  hasAdminPages: boolean;
  shortcuts: boolean;
  onOpen: (opening: Opening) => void;
  onClose: () => void;
}) {
  const [recents, setRecents] = useState<FoundTitle[]>([]);
  const navigate = useNavigate();
  const pages = useMemo(() => pagesFor(hasAdminPages), [hasAdminPages]);
  const onJump = useCallback(
    (letter: string) => {
      const page = pages.find((each) => each.letter === letter);
      if (page !== undefined) {
        void navigate({ to: page.to });
      }
    },
    [pages, navigate],
  );
  useActionBarKeys({ enabled: shortcuts && opening === undefined, onOpen, onJump });
  if (opening === undefined) {
    return null;
  }
  return (
    <Suspense fallback={null}>
      <ActionBarDialog
        key={`${opening.frame}/${opening.query}`}
        opening={opening}
        pages={pages}
        recents={recents}
        onOpenTitle={(title) => {
          setRecents((before) =>
            [title, ...before.filter(({ key }) => key !== title.key)].slice(0, 4),
          );
        }}
        onClose={onClose}
      />
    </Suspense>
  );
}
