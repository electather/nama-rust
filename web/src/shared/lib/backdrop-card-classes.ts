import { backdropTile } from "@/shared/lib/tile-shapes";

/** The link that covers a backdrop card and names it, with its focus ring inside the card's edge. */
export const backdropLink = `absolute inset-0 outline-none focus-visible:outline-2 focus-visible:-outline-offset-3 focus-visible:outline-ring ${backdropTile.radius}`;
