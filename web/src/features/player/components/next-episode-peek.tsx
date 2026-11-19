import { EpisodeStill } from "@/features/player/components/episode-still";
import { EpisodeText } from "@/features/player/components/episode-text";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/**
 * The next Episode as its row in the Episodes panel shows it, over the Next Episode button, while
 * the button is hovered or focused from the keyboard. It only previews what the button opens, so
 * it's the button's description and takes no pointer.
 */
export function NextEpisodePeek({ id, episode }: { id: string; episode: Episode }) {
  return (
    <span
      id={id}
      role="tooltip"
      className="pointer-events-none absolute inset-s-0 bottom-full mb-12 hidden w-max max-w-80 items-center gap-3 rounded-lg border bg-popover p-3 text-popover-foreground shadow-md group-hover/next:flex group-has-focus-visible/next:flex"
    >
      <EpisodeStill episode={episode} played={undefined} />
      <EpisodeText episode={episode} playing={false} />
    </span>
  );
}
