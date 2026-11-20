import { FeaturedArt } from "@/features/home/components/featured-art";
import { FeaturedCopy } from "@/features/home/components/featured-copy";
import { driftOf, openGrow, tiltOf } from "@/features/home/lib/accordion";
import type { Panel } from "@/features/home/lib/featured-panels";

declare module "react" {
  interface CSSProperties {
    /** How much of the hero's row a Featured panel takes, against the others. */
    "--grow"?: number;
    /** How a closed Featured panel turns toward the open one. */
    "--tilt"?: string;
    /** Where a Featured panel's art sits across it, drifting from its centre while closed. */
    "--drift"?: string;
  }
}

/**
 * One Featured title's panel: open, its art in colour under its copy; closed, a grey sliver
 * turned toward the open one, which opens it when hovered, focused or pressed.
 */
export function FeaturedPanel({
  panel,
  place: { index, open, count },
  onOpen,
}: {
  panel: Panel;
  place: { index: number; open: number; count: number };
  onOpen: () => void;
}) {
  const offset = index - open;
  const isOpen = offset === 0;
  return (
    <li
      data-open={isOpen ? "" : undefined}
      className="group relative isolate min-w-0 shrink grow-(--grow) basis-0 transform-(--tilt) overflow-hidden rounded-3xl bg-muted shadow-panel transition-all duration-600 ease-out-expo motion-reduce:transition-none max-[720px]:min-h-21 max-[720px]:transform-none max-[720px]:data-open:min-h-130"
      style={{
        "--grow": isOpen ? openGrow(count) : 1,
        "--tilt": tiltOf(offset),
        "--drift": `calc(-50% + ${driftOf(offset)})`,
      }}
    >
      <FeaturedArt panel={panel} first={index === 0} />
      {/* The whole panel is its button, so hovering anywhere on it opens it. It opens on hover and
          focus, never navigates, so it keeps the arrow the app's buttons otherwise swap for a hand. */}
      <button
        type="button"
        aria-label={`Show ${panel.title}`}
        aria-current={isOpen ? "true" : undefined}
        onClick={onOpen}
        onFocus={onOpen}
        onMouseEnter={onOpen}
        className="absolute inset-0 cursor-default rounded-3xl outline-none focus-visible:outline-2 focus-visible:-outline-offset-3 focus-visible:outline-ring"
      />
      <FeaturedCopy panel={panel} />
    </li>
  );
}
