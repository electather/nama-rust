import { Button } from "nama-ui/components/button";
import type { ReactNode } from "react";

import type { ToMatch } from "@/shared/lib/find-a-match";

/**
 * What Matching does, with the one primary Match button at its end, `cancel` before it, and what
 * the last try said: a `notice` that it Matched but not as hoped, or why it `refusal`. It stays at
 * the foot of a tall page, with a hairline over it.
 */
export function MatchFooter({
  item,
  cancel,
  disabled,
  notice,
  refusal,
}: {
  item: ToMatch["item"];
  cancel: ReactNode;
  disabled: boolean;
  notice: string;
  refusal: string | undefined;
}) {
  const caption =
    "matchedTitle" in item
      ? "Every file of this title is Matched to the one you pick."
      : "Once Matched, it gets its poster and facts, and shows in its Library.";
  return (
    <footer className="sticky bottom-0 mt-auto flex flex-col gap-2 border-t bg-inherit py-3.5">
      <output className="block text-sm text-pretty empty:hidden">{notice}</output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {refusal}
      </p>
      {/* A phone's column reads Match, then Close, then what Matching does. */}
      <div className="flex items-center gap-2.5 max-md:flex-col-reverse max-md:items-stretch">
        <p className="flex-1 text-xs text-pretty text-muted-foreground">{caption}</p>
        {cancel}
        <Button type="submit" size="touch" disabled={disabled}>
          Match
        </Button>
      </div>
    </footer>
  );
}
