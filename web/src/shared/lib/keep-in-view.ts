/** Scrolls the option of a list just picked, given as a `ref`, into the list's view. */
export function keepInView(option: HTMLElement | null) {
  option?.scrollIntoView({ block: "nearest" });
}
