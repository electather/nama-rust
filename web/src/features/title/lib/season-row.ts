import type { Episode } from "@/gen/nama/v1/browse_pb";

// The row keeps only the cards near the view in the page, and every card is as wide as the next,
// so these count card widths from one it shows rather than finding a card.
// ponytail: measures left to right; a right-to-left language would measure from the right.

/** What scrolls a Show's row: to its card at `index`, put at its start, as far as the row goes. */
export interface RowScroller {
  toCard: (index: number, behavior: ScrollBehavior) => void;
}

/** The width of a Show's row's cards and how far in from its edges they sit, in pixels. */
export interface RowBox {
  width: number;
  inset: number;
}

/**
 * The width of the row's cards, from one it shows, and how far in from its edges they sit, as its
 * snapping inset; nothing while it shows no card.
 */
function rowBox(row: HTMLElement): RowBox | undefined {
  const width = row.querySelector<HTMLElement>("[data-season]")?.offsetWidth;
  const inset = Number(getComputedStyle(row).scrollPaddingInlineStart.replace("px", ""));
  return width === undefined ? undefined : { width, inset: Number.isNaN(inset) ? 0 : inset };
}

/**
 * Sets the `row`'s `RowBox` now and whenever the row's size changes, where a browser can tell.
 * Returns what stops it.
 */
export function watchBox(row: HTMLElement, setBox: (box: RowBox) => void) {
  const update = () => {
    const box = rowBox(row);
    if (box) {
      setBox(box);
    }
  };
  update();
  const observer = typeof ResizeObserver === "function" ? new ResizeObserver(update) : undefined;
  observer?.observe(row);
  return () => observer?.disconnect();
}

/** How far apart the row's cards start, a card's width and the gap after it, in pixels. */
function cardStep(row: HTMLElement) {
  const card = row.querySelector<HTMLElement>("[data-season]");
  return card ? card.offsetWidth + gapAfter(card) : undefined;
}

/** The gap between `card` and the next, in pixels, as its list's computed style says. */
function gapAfter(card: HTMLElement) {
  const gap = Number(getComputedStyle(card.parentElement ?? card).columnGap.replace("px", ""));
  return Number.isNaN(gap) ? 0 : gap;
}

/**
 * The Season of the card the row of `episodes` has snapped to, or of the last card once the row is
 * scrolled to its end, where a short last Season's first card can't reach the start.
 */
export function seasonInView(row: HTMLElement, episodes: Episode[]) {
  const step = cardStep(row);
  return step === undefined
    ? undefined
    : episodes[snappedIndex(row, step, episodes.length)]?.seasonNumber;
}

/** The index of the card the row has snapped to, cards `step` apart, or its last, of `count`, at its end. */
function snappedIndex(row: HTMLElement, step: number, count: number) {
  const atEnd = row.scrollLeft + row.clientWidth >= row.scrollWidth - 1;
  return atEnd ? count - 1 : Math.round(row.scrollLeft / step);
}

/** The index in a Show's row of `season`'s first Episode, or -1 for a Season it doesn't have. */
export function firstOfSeason(episodes: Episode[], season: number) {
  return episodes.findIndex(({ seasonNumber }) => seasonNumber === season);
}

/**
 * Has `jump` scroll the `row` with snapping off till the frame after next. A jump past the cards the
 * row shows would snap back to the nearest one still in the page, so it snaps again only once the
 * virtualizer shows the cards it jumped to.
 */
export function jumpUnsnapped(row: HTMLElement, jump: () => void) {
  row.style.scrollSnapType = "none";
  jump();
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      row.style.scrollSnapType = "";
    });
  });
}

/** Smooth, unless reduced motion is asked for. */
export function smoothUnlessReduced(): ScrollBehavior {
  return globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth";
}
