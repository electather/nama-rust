/** Opens a dialog whose trigger lives elsewhere, such as an item of a menu that closes on choosing. */
export interface Opener {
  open: () => void;
}
