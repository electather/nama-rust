/** A backdrop card's corners, which its art's hairline and its link's focus ring follow. */
export const backdropRadius = "rounded-2xl";

/** The link that covers a backdrop card and names it, with its focus ring inside the card's edge. */
export const backdropLink = `absolute inset-0 outline-none focus-visible:outline-2 focus-visible:-outline-offset-3 focus-visible:outline-ring ${backdropRadius}`;
