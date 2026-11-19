import type { RefObject } from "react";

/** Where the player's panels open: inside the player, so they show in full screen, over the bar. */
export interface PanelPlace {
  container: RefObject<HTMLDivElement | null>;
  /** The bar's rows, whose top end the panels sit above, as wide as the bar. */
  anchor: RefObject<HTMLDivElement | null>;
}

/** How far over the bar's rows a panel sits, in pixels. */
export const PANEL_GAP = 14;
