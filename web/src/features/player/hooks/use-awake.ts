import { useCallback, useEffect, useRef, useState } from "react";

/** How long the controls stay after the pointer or keyboard was last used, while playing. */
const CONTROLS_LINGER_MS = 3000;

/** Whether the pointer or keyboard was used lately, and `wake` to say it just was. */
export function useAwake() {
  const [awake, setAwake] = useState(true);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const sleepLater = useCallback(() => {
    globalThis.clearTimeout(timer.current);
    timer.current = globalThis.setTimeout(() => {
      setAwake(false);
    }, CONTROLS_LINGER_MS);
  }, []);
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
