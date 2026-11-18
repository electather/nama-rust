import { useSyncExternalStore } from "react";

/** `posterGrid`'s breakpoints, widest first, as Tailwind's `lg`, `md` and `sm` place them. */
const wider = ["(min-width: 64rem)", "(min-width: 48rem)", "(min-width: 40rem)"];

function subscribe(changed: () => void) {
  const queries = wider.map((query) => matchMedia(query));
  for (const query of queries) {
    query.addEventListener("change", changed);
  }
  return () => {
    for (const query of queries) {
      query.removeEventListener("change", changed);
    }
  };
}

/** How many columns `posterGrid` has at the window's width now: 5, 4, 3 or 2. */
function posterColumns() {
  const widest = wider.findIndex((query) => matchMedia(query).matches);
  return widest === -1 ? 2 : 5 - widest;
}

/** How many columns `posterGrid` has now, following the window's width. */
export function usePosterColumns() {
  return useSyncExternalStore(subscribe, posterColumns, () => 5);
}
