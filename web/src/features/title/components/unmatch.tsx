import { useNavigate } from "@tanstack/react-router";
import type { Ref } from "react";

import { useClearTitleMatch } from "@/features/title/data/use-clear-title-match";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import type { TitleToMatch } from "@/shared/lib/find-a-match";
import { refusal } from "@/shared/lib/forms";
import type { Opener } from "@/shared/lib/opener";
import { withYear } from "@/shared/lib/unmatched-names";

/**
 * The dialog that Unmatches every Item of the Movie or Show `title`, which puts its files on Needs
 * a Match, then goes to its Library. Only `ref` opens it, from the "…" menu.
 */
export function Unmatch({ ref, title }: { ref: Ref<Opener>; title: TitleToMatch }) {
  const { matchedTitle } = title;
  const clear = useClearTitleMatch(matchedTitle.libraryId);
  const navigate = useNavigate();
  return (
    <ConfirmDialog
      ref={ref}
      trigger={false}
      // Matching it again undoes it.
      destructive={false}
      action="Unmatch"
      name="Unmatch"
      title={`Unmatch ${withYear(title.title, title.year)}?`}
      description="Its files go to Needs a Match, where you can Match them again. Until then, nobody finds it in the Library."
      onConfirm={async () => {
        await clear.mutateAsync({ matchedTitle });
        await navigate({
          to: "/libraries/$libraryId",
          params: { libraryId: String(matchedTitle.libraryId) },
        });
      }}
      refusal={(error) => refusal(error, "unmatch it", {}).form}
    />
  );
}
