/** Calls `then` once the web font has loaded, which changes how much room text takes. */
export function afterFontsLoad(then: () => void) {
  // happy-dom, which the tests run on, has no `document.fonts`.
  if ("fonts" in document) {
    const callOnceLoaded = async () => {
      await document.fonts.ready;
      then();
    };
    void callOnceLoaded();
  }
}
