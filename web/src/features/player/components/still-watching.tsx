import type { LinkOptions } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";
import { useEffect, useId, useRef } from "react";

import { BackLink } from "@/features/player/components/back-link";
import { useGoBack } from "@/features/player/hooks/use-go-back";

/** "Are you still watching?", which keeps the player paused until it's answered. */
export function StillWatching({
  back,
  onKeepWatching,
}: {
  back: LinkOptions;
  onKeepWatching: () => void;
}) {
  const goBack = useGoBack(back);
  const titleId = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  return (
    // A modal `<dialog>` keeps the pointer and focus off the paused controls until it's answered,
    // and the player's shortcuts leave keys to it.
    <dialog
      ref={dialog}
      role="alertdialog"
      aria-labelledby={titleId}
      // Esc goes Back, as the player's shortcut does, and doesn't just close it.
      onCancel={(event) => {
        event.preventDefault();
        goBack();
      }}
      // Floats above the player, so it gets a shadow (DESIGN.md, "Elevation").
      className="m-auto w-full max-w-xs rounded-xl border bg-popover p-6 text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-sm"
    >
      <h2 id={titleId} className="text-lg font-semibold tracking-tight">
        Are you still watching?
      </h2>
      <div className="mt-6 flex gap-2">
        <Button onClick={onKeepWatching}>Keep watching</Button>
        <BackLink back={back} variant="ghost">
          Back
        </BackLink>
      </div>
    </dialog>
  );
}
