import { useEffect, useState } from "react";

/** Whether `on` has been true for `ms` already: a wait long enough to be worth showing. */
export function useLate(on: boolean, ms: number) {
  const [late, setLate] = useState(false);
  useEffect(() => {
    const timer = on
      ? setTimeout(() => {
          setLate(true);
        }, ms)
      : undefined;
    return () => {
      clearTimeout(timer);
      setLate(false);
    };
  }, [on, ms]);
  return on && late;
}
