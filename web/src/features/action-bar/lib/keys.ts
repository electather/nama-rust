/** What each key does, by `KeyboardEvent.key`. A key left out, or `undefined`, is left alone. */
export type Keys = Record<string, (() => void) | undefined>;

/** Runs what `keys` has for the key pressed, and keeps the browser from also acting on it. */
export function press(event: { key: string; preventDefault: () => void }, keys: Keys) {
  const run = keys[event.key];
  if (run !== undefined) {
    event.preventDefault();
    run();
  }
}
