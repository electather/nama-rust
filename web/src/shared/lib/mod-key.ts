/** How a shortcut's modifier key is written here: ⌘ on an Apple platform, Ctrl elsewhere. */
export const modKey = /Mac|iPhone|iPad/u.test(globalThis.navigator.userAgent) ? "⌘" : "Ctrl";
