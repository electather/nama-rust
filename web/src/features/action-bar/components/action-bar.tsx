import { type RefObject, useEffect, useId } from "react";

import { ActionBarFooter } from "@/features/action-bar/components/action-bar-footer";
import { ActionBarHeader } from "@/features/action-bar/components/action-bar-header";
import { ActionBarList } from "@/features/action-bar/components/action-bar-list";
import { useActionBar } from "@/features/action-bar/hooks/use-action-bar";
import { useLightDismiss } from "@/features/action-bar/hooks/use-light-dismiss";
import type { Opening } from "@/features/action-bar/lib/rows";
import { TitleSourceNote } from "@/shared/components/title-source-note";

/**
 * The action bar: the one place to search your Libraries, jump to a page and see the keyboard
 * shortcuts. It opens as it mounts, as `opening` says, and calls `onClose` when it has closed, for its
 * owner to unmount it: the next opening starts afresh at its root.
 */
export function ActionBar({
  ref,
  opening,
  hasAdminPages,
  onClose,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  opening: Opening;
  hasAdminPages: boolean;
  onClose: () => void;
}) {
  const bar = useActionBar({
    opening,
    hasAdminPages,
    dialog: {
      close: () => ref.current?.close(),
      focusField: () => ref.current?.querySelector("input")?.focus(),
    },
  });
  const listId = useId();
  useEffect(() => {
    // Strict Mode runs this twice, and a dialog that's open throws on a second `showModal()`.
    if (ref.current?.open === false) {
      ref.current.showModal();
    }
  }, [ref]);
  useLightDismiss(ref);
  return (
    // ponytail: the native modal <dialog> traps focus, closes on Escape and the back gesture, and
    // returns focus itself. Its backdrop and it contain scrolling, so the page behind stays put.
    // oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- keys pressed on the controls inside bubble here
    <dialog
      ref={ref}
      aria-label="Command menu"
      // Not in React's types yet: a press on the backdrop closes it, as Escape does.
      {...{ closedby: "any" }}
      onClose={onClose}
      onKeyDown={bar.handleKeyDown}
      className="inset-x-4 mx-auto mt-24 mb-auto max-h-3/4 w-auto max-w-160 flex-col overflow-hidden overscroll-contain rounded-xl border bg-popover p-1 text-popover-foreground shadow-lg duration-150 ease-out-expo outline-none backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim/15 backdrop:backdrop-blur-xs open:flex open:animate-in open:fade-in-0 open:zoom-in-95 motion-reduce:animate-none"
    >
      <ActionBarHeader bar={bar} listId={listId} />
      <ActionBarList bar={bar} listId={listId} />
      <TitleSourceNote shown={bar.titleSourceUnavailable} className="px-3 pb-2 text-xs empty:p-0" />
      <ActionBarFooter />
    </dialog>
  );
}
