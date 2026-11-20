/**
 * Focuses the element it's given, as a ref: a notice takes the focus its card's menu lost with
 * the card. Stable, so React calls it only when the element mounts.
 */
export const focusOnMount = (element: HTMLElement | null) => element?.focus();
