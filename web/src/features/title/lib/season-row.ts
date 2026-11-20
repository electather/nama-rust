// These read each card's Season from its `data-season`, so the row's scroll needs no React state.
// ponytail: measures left to right; a right-to-left language would measure from the right.

/**
 * The Season of the card at the row's start, past half its width, or of the last card once the row
 * is scrolled to its end, where a short last Season's first card can't reach the start.
 */
export function seasonInView(row: HTMLElement) {
  const cards = [...row.querySelectorAll<HTMLElement>("[data-season]")];
  const atEnd = row.scrollLeft + row.clientWidth >= row.scrollWidth - 1;
  const start = row.getBoundingClientRect().left + scrollPaddingOf(row);
  const card = atEnd
    ? cards.at(-1)
    : cards.find((shown) => shown.getBoundingClientRect().right > start + shown.offsetWidth / 2);
  return card === undefined ? undefined : Number(card.dataset["season"]);
}

/** How far in from its start edge the row snaps its cards, in pixels, as its computed style says. */
function scrollPaddingOf(row: HTMLElement) {
  const padding = Number(getComputedStyle(row).scrollPaddingInlineStart.replace("px", ""));
  return Number.isNaN(padding) ? 0 : padding;
}

/** Scrolls the row to put `season`'s first card at its start, as far as the row goes. */
export function scrollToSeason(row: HTMLElement, season: number, behavior: ScrollBehavior) {
  const first = row.querySelector("[data-season]");
  const target = row.querySelector(`[data-season="${season}"]`);
  if (first && target) {
    const left = target.getBoundingClientRect().left - first.getBoundingClientRect().left;
    row.scrollTo({ left, behavior });
  }
}

/** Smooth, unless reduced motion is asked for. */
export function smoothUnlessReduced(): ScrollBehavior {
  return globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth";
}

/**
 * Scrolls `row`, a row of chips that scrolls sideways, just enough to show `chip` inside its
 * snapping inset, and only sideways, so the page itself never moves.
 */
export function revealInRow(row: HTMLElement, chip: HTMLElement) {
  const inset = scrollPaddingOf(row);
  const shown = row.getBoundingClientRect();
  const box = chip.getBoundingClientRect();
  const before = box.left - (shown.left + inset);
  const after = box.right - (shown.right - inset);
  if (before < 0) {
    row.scrollBy({ left: before, behavior: smoothUnlessReduced() });
  } else if (after > 0) {
    row.scrollBy({ left: after, behavior: smoothUnlessReduced() });
  }
}
