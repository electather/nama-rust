/** The rows of `list`, its child elements, or none before it's drawn. */
export function listRows(list: HTMLElement | null) {
  return [...(list?.children ?? [])];
}
