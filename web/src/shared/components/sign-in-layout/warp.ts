import { createContext, use, useEffect } from "react";

/** Sets whether the signed-out layout's light rushes. Outside that layout there's none. */
export const WarpContext = createContext<((on: boolean) => void) | null>(null);

/** Makes the signed-out layout's light rush while `on`, such as while waiting on the Server. */
export function useWarp(on: boolean) {
  const setWarp = use(WarpContext);
  useEffect(() => {
    setWarp?.(on);
    return () => {
      setWarp?.(false);
    };
  }, [setWarp, on]);
}
