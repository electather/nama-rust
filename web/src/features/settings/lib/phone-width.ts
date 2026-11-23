/** Whether Settings is laid out for a phone, under `md` (48rem), where focus on Search opens the keyboard. */
export function phoneWidth() {
  return !globalThis.matchMedia("(min-width: 48rem)").matches;
}
