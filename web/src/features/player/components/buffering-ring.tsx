import { Spinner } from "@/features/player/components/spinner";

/** The spinner over the middle of the video while it's `buffering`, waiting for more of it. */
export function BufferingRing({ buffering }: { buffering: boolean }) {
  if (!buffering) {
    return null;
  }
  return (
    <output className="pointer-events-none absolute inset-s-1/2 top-1/2 grid size-18 -translate-1/2 place-items-center rounded-full bg-scrim">
      <span className="sr-only">Buffering</span>
      <Spinner />
    </output>
  );
}
