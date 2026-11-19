import { type RefObject, useCallback, useEffect, useRef, useState } from "react";

/** How long the controls stay after the pointer or keyboard was last used, while playing. */
const CONTROLS_LINGER_MS = 3000;

// A panel the controls opened, which keeps them up until it closes.
const OPEN_PANEL = '[data-slot="popover-content"]';

/**
 * Whether the pointer or keyboard was used lately, and `wake` to say it just was. While `player`
 * holds an open panel, it stays awake.
 */
export function useAwake(playerRef: RefObject<HTMLElement | null>) {
  const [awake, setAwake] = useState(true);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const sleepLater = useCallback(() => {
    globalThis.clearTimeout(timer.current);
    timer.current = globalThis.setTimeout(function sleep() {
      if (playerRef.current?.querySelector(OPEN_PANEL)) {
        timer.current = globalThis.setTimeout(sleep, CONTROLS_LINGER_MS);
        return;
      }
      setAwake(false);
    }, CONTROLS_LINGER_MS);
  }, [playerRef]);
  const wake = useCallback(() => {
    setAwake(true);
    sleepLater();
  }, [sleepLater]);
  useEffect(() => {
    sleepLater();
    return () => {
      globalThis.clearTimeout(timer.current);
    };
  }, [sleepLater]);
  return [awake, wake] as const;
}
