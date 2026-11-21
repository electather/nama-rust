import { Spinner } from "@/features/player/components/spinner";

/**
 * The spinner over the middle of the video while it's `buffering`, waiting for more of it. It fades
 * in only after a moment, since an HLS seek into what's loaded waits for some 30 ms.
 */
export function BufferingRing({ buffering }: { buffering: boolean }) {
  if (!buffering) {
    return null;
  }
  return (
    <output className="pointer-events-none absolute inset-s-1/2 top-1/2 grid size-18 -translate-1/2 place-items-center rounded-full bg-scrim transition-opacity delay-300 duration-150 starting:opacity-0">
      <span className="sr-only">Buffering</span>
      <Spinner />
    </output>
  );
}
