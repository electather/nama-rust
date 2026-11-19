import { useNavigate } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import { type Keys, press } from "@/features/action-bar/lib/keys";
import { pagesFor } from "@/features/action-bar/lib/pages";
import type { Frame } from "@/features/action-bar/lib/rows";

/** Whether ⌘ or Ctrl is held, which makes a key a command rather than a letter. */
function held(event: KeyboardEvent) {
  return event.metaKey || event.ctrlKey;
}

/** Whether a plain key is text being typed, or a browser's own Alt shortcut, not one of ours. */
function quiet({ altKey, target }: KeyboardEvent) {
  const typing =
    target instanceof HTMLElement &&
    (target.isContentEditable || target.matches("input, textarea, select"));
  return altKey || typing;
}

/**
 * The keys that reach the action bar from anywhere on a signed-in page: ⌘K or Ctrl K opens or
 * closes it, / opens it, ⌘/ opens its keyboard shortcuts, and G then a page's key goes to that
 * page. `lib/shortcuts.ts` lists them for the keyboard shortcuts frame.
 */
export function useActionBarKeys({
  dialog,
  hasAdminPages,
  open,
}: {
  /** The action bar's dialog, while it's open. */
  dialog: RefObject<HTMLDialogElement | null>;
  hasAdminPages: boolean;
  open: (frame: Frame) => void;
}) {
  const navigate = useNavigate();
  useEffect(() => {
    // When G was last pressed: a page's key goes to its page only within a second of it.
    let armed = Number.NEGATIVE_INFINITY;
    const jumps: Keys = Object.fromEntries(
      pagesFor(hasAdminPages).map((page) => [
        page.key.toLowerCase(),
        () => {
          void navigate({ to: page.to });
        },
      ]),
    );
    const commands: Keys = {
      k: () => {
        open("root");
      },
      "/": () => {
        open("shortcuts");
      },
    };
    const whileOpen: Keys = { k: () => dialog.current?.close() };

    function plainKeys(event: KeyboardEvent, sinceG: number): Keys {
      const always: Keys = {
        g: () => {
          armed = event.timeStamp;
        },
        "/": () => {
          open("root");
        },
      };
      return sinceG < 1000 ? { ...jumps, ...always } : always;
    }

    function keysWhileClosed(event: KeyboardEvent, sinceG: number): Keys {
      // Another dialog, such as "Confirm it’s you", keeps the keys to itself.
      if (document.querySelector("dialog[open]") !== null) {
        return {};
      }
      if (held(event)) {
        return commands;
      }
      return quiet(event) ? {} : plainKeys(event, sinceG);
    }

    function onKeyDown(event: KeyboardEvent) {
      const sinceG = event.timeStamp - armed;
      // Any key but G itself ends the wait for a page's key.
      armed = Number.NEGATIVE_INFINITY;
      if (dialog.current?.open === true) {
        // The dialog takes its own keys (`use-action-bar.ts`): only the way out is ours.
        press(event, held(event) ? whileOpen : {});
      } else {
        press(event, keysWhileClosed(event, sinceG));
      }
    }

    globalThis.addEventListener("keydown", onKeyDown);
    return () => {
      globalThis.removeEventListener("keydown", onKeyDown);
    };
  }, [dialog, hasAdminPages, navigate, open]);
}
