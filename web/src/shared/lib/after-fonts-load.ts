/**
 * Calls `then` once the web font has loaded, which changes how much room text takes, unless the
 * function it returns is called first, as an effect's cleanup.
 */
export function afterFontsLoad(then: () => void): () => void {
  let cancelled = false;
  // happy-dom, which the tests run on, has no `document.fonts`.
  if ("fonts" in document) {
    const callOnceLoaded = async () => {
      await document.fonts.ready;
      if (!cancelled) {
        then();
      }
    };
    void callOnceLoaded();
  }
  return () => {
    cancelled = true;
  };
}
