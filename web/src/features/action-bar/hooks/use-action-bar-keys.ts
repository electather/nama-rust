import { useEffect } from "react";

import type { Frame, Opening } from "@/features/action-bar/lib/menu";

/** How long after G its page's letter still jumps. */
const sequenceMs = 1500;

const withMod: Partial<Record<string, Frame>> = { k: "root", "/": "shortcuts" };
const alone: Partial<Record<string, Frame>> = { "/": "root" };

/** Whether keys pressed in `target` are being typed, so a plain letter isn't a shortcut. */
function typingIn(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName))
  );
}

/** Whether `event` is a key pressed alone, outside a field. */
function plain(event: KeyboardEvent) {
  return !(event.metaKey || event.ctrlKey || event.altKey) && !typingIn(event.target);
}

function opensOn(event: KeyboardEvent) {
  if (event.metaKey || event.ctrlKey) {
    return withMod[event.key.toLowerCase()];
  }
  return plain(event) ? alone[event.key] : undefined;
}

/** Whether `event` is a page's letter, pressed in time after G was, at `pressedG`. */
function afterG(event: KeyboardEvent, pressedG: number) {
  return plain(event) && event.timeStamp - pressedG < sequenceMs;
}

function startsJump(event: KeyboardEvent) {
  return plain(event) && event.key.toLowerCase() === "g";
}

/**
 * The shortcuts that open the action bar from anywhere while it's closed: ⌘K or Ctrl K, ⌘/ for the
 * shortcuts list, and / when not typing. G then a page's letter calls `onJump` with the letter.
 */
export function useActionBarKeys({
  enabled,
  onOpen,
  onJump,
}: {
  enabled: boolean;
  onOpen: (opening: Opening) => void;
  onJump: (letter: string) => void;
}) {
  useEffect(() => {
    let pressedG = Number.NEGATIVE_INFINITY;
    const onKeyDown = (event: KeyboardEvent) => {
      const frame = opensOn(event);
      if (frame !== undefined) {
        event.preventDefault();
        onOpen({ frame, query: "" });
      } else if (afterG(event, pressedG)) {
        onJump(event.key.toUpperCase());
      }
      pressedG = startsJump(event) ? event.timeStamp : Number.NEGATIVE_INFINITY;
    };
    const listening = new AbortController();
    if (enabled) {
      globalThis.addEventListener("keydown", onKeyDown, {
        capture: true,
        signal: listening.signal,
      });
    }
    return () => {
      listening.abort();
    };
  }, [enabled, onOpen, onJump]);
}
