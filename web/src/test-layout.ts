import { vi } from "vite-plus/test";

/**
 * Lays out rows that scroll sideways, such as Home's shelves and a Show's Episodes, as a browser would and happy-dom doesn't: each scrolling element
 * 1,400px wide and each tile 100px, so a virtual shelf shows its tiles. Without a ResizeObserver
 * the virtualizer reads these sizes straight off the elements. `vi.restoreAllMocks` and
 * `vi.unstubAllGlobals` undo it.
 */
export function layOutShelves() {
  // oxlint-disable-next-line unicorn/no-useless-undefined -- `stubGlobal` takes the value to stub, here none
  vi.stubGlobal("ResizeObserver", undefined);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1400);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function width(
    this: HTMLElement,
  ) {
    return this.tagName === "LI" ? 100 : 1400;
  });
}
