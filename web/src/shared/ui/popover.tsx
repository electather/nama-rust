import { Popover as PopoverPrimitive } from "@base-ui/react/popover";
import { cva, type VariantProps } from "class-variance-authority";

// The player's panels, as its artboards draw them: Popover with a hairline and `shadow-md`. A panel
// grows out of its button on an exponential ease-out and shrinks back into it speeding up, and
// quicker: closing on the ease-out is gone in 50ms, and on its exact reverse holds still for 100ms,
// and both look cut. Transitions rather than keyframes, so a panel closed while it opens turns back
// from there. One that changes size while open, as Playback settings does opening a list, eases
// from the old size to the new (`resizeSmoothly`).

function Popover(props: PopoverPrimitive.Root.Props) {
  return <PopoverPrimitive.Root data-slot="popover" {...props} />;
}

function PopoverTrigger(props: PopoverPrimitive.Trigger.Props) {
  return <PopoverPrimitive.Trigger data-slot="popover-trigger" {...props} />;
}

const popoverContentVariants = cva(
  "z-50 flex max-h-(--available-height) max-w-(--available-width) origin-(--from-button) flex-col overflow-hidden rounded-lg border bg-popover text-sm text-popover-foreground shadow-md transition duration-150 ease-out-expo outline-none data-ending-style:scale-95 data-ending-style:opacity-0 data-ending-style:duration-100 data-ending-style:ease-in data-resizing:transition-all data-resizing:duration-250 data-starting-style:scale-95 data-starting-style:opacity-0 motion-reduce:transition-none",
  {
    variants: {
      width: {
        // A menu of settings.
        menu: "w-75",
        // A panel of text, such as Playback info.
        text: "w-100",
        // A list of rich rows, such as the Episodes of a Season.
        list: "w-90",
        // Two lists side by side.
        double: "w-135",
      },
    },
    defaultVariants: { width: "menu" },
  },
);

/**
 * Puts `--from-button` on the panel in `positioner`: the middle of the button that opened it, so
 * the panel grows out of it. Base UI's own `--transform-origin` is the middle of the `anchor`, which
 * for the bottom bar's panels is the whole bar.
 */
function growFromButton(positioner: HTMLDivElement, popup: HTMLElement) {
  const button = document.querySelector(`[aria-controls="${popup.id}"]`);
  if (!button) {
    return;
  }
  const { left, top, width, height } = button.getBoundingClientRect();
  // The positioner's box, as the panel itself is already scaled down to grow.
  const box = positioner.getBoundingClientRect();
  const [x, y] = [left + width / 2 - box.left, top + height / 2 - box.top];
  popup.style.setProperty("--from-button", `${x}px ${y}px`);
}

/** How long a panel takes to change size: `data-resizing:duration-250`. */
const RESIZE_MS = 250;

interface Size {
  width: number;
  height: number;
}

/** The panel's size as laid out, not as scaled while it opens. */
function sizeOf(popup: HTMLElement): Size {
  return { width: popup.offsetWidth, height: popup.offsetHeight };
}

function sizeIn(popup: HTMLElement, { width, height }: Size) {
  popup.style.width = `${width}px`;
  popup.style.height = `${height}px`;
}

/**
 * Eases `popup` from its size to the one it now lays out at whenever what it shows changes size.
 * Base UI does this only for one panel moving between buttons (`Popover.Viewport`). A size that
 * changes while it eases waits for it to end, then eases on from there.
 */
function resizeSmoothly(popup: HTMLElement) {
  let settled = sizeOf(popup);
  const observer = new ResizeObserver(() => {
    const now = sizeOf(popup);
    if (Object.hasOwn(popup.dataset, "resizing") || sameSize(settled, now)) {
      return;
    }
    if (!globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      ease(popup, settled, now);
    }
    settled = now;
  });
  observer.observe(popup);
  return () => {
    observer.disconnect();
  };
}

function sameSize(a: Size, b: Size) {
  return Math.abs(a.width - b.width) < 1 && Math.abs(a.height - b.height) < 1;
}

// Back to the old size, laid out before the transition is on, as setting it with the transition on
// would start one there; then to the new size, and to what the content gives once there.
function ease(popup: HTMLElement, from: Size, to: Size) {
  sizeIn(popup, from);
  popup.getBoundingClientRect();
  popup.dataset["resizing"] = "";
  sizeIn(popup, to);
  setTimeout(() => {
    delete popup.dataset["resizing"];
    popup.style.removeProperty("width");
    popup.style.removeProperty("height");
  }, RESIZE_MS);
}

/**
 * Sets the panel in `positioner` growing out of its button and easing its size, until the cleanup
 * it returns. One function for every render, so React calls it only when the panel mounts: a new
 * one each render would start again from the size the panel just changed to.
 */
function onPositioner(positioner: HTMLDivElement | null) {
  const popup = positioner?.querySelector<HTMLElement>("[data-slot=popover-content]");
  return positioner && popup ? fromButtonAndResizing(positioner, popup) : undefined;
}

function fromButtonAndResizing(positioner: HTMLDivElement, popup: HTMLElement) {
  growFromButton(positioner, popup);
  // Without a ResizeObserver, a panel changes size at once.
  return typeof ResizeObserver === "function" ? resizeSmoothly(popup) : undefined;
}

// Clear of the player's title bar above and its bottom bar below, whichever the panel opens toward.
const collisionPadding = {
  top: { top: 100, right: 20, bottom: 20, left: 20 },
  bottom: { top: 20, right: 20, bottom: 120, left: 20 },
};

/**
 * The panel, in `container` so it shows in full screen, on `side` of `anchor`'s end edge,
 * `sideOffset` pixels off it.
 */
function PopoverContent({
  container,
  anchor,
  side = "top",
  sideOffset = 4,
  width,
  ...props
}: PopoverPrimitive.Popup.Props &
  VariantProps<typeof popoverContentVariants> &
  Pick<PopoverPrimitive.Positioner.Props, "anchor" | "sideOffset"> & {
    side?: "top" | "bottom";
  } & Pick<PopoverPrimitive.Portal.Props, "container">) {
  return (
    <PopoverPrimitive.Portal container={container}>
      <PopoverPrimitive.Positioner
        ref={onPositioner}
        className="isolate z-50 outline-none"
        side={side}
        align="end"
        anchor={anchor}
        sideOffset={sideOffset}
        collisionPadding={collisionPadding[side]}
        // A tall list shrinks to fit rather than moving beside its button.
        collisionAvoidance={{ side: "none" }}
      >
        <PopoverPrimitive.Popup
          data-slot="popover-content"
          className={popoverContentVariants({ width })}
          {...props}
        />
      </PopoverPrimitive.Positioner>
    </PopoverPrimitive.Portal>
  );
}

export { Popover, PopoverContent, PopoverTrigger };
