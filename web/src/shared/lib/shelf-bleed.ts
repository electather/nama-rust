/**
 * Room above and below a row of tiles that scroll sideways, for a lifted tile and its shadow
 * (`tileMotion`) that its scroller would clip. The negative margins take back what the padding
 * adds, so the page lays out as if the row had `pt-1 pb-2`.
 */
export const shelfLift = "-mt-1 -mb-5 pt-2 pb-7";

/**
 * A shelf's row, out past the page's column to the screen's edges, inset to line its first
 * tile up with the column (`src/styles.css`).
 */
export const shelfBleed = `mx-bleed px-shelf-edge ${shelfLift}`;
