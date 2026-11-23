import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { PlayProgress } from "@/shared/components/backdrop-card/play-progress";
import { TitleOrLogo } from "@/shared/components/title-or-logo";

/**
 * A card's Title Logo: up to 48px tall and 60% of the card wide (DESIGN.md, "Backdrop cards"), with
 * room under it before the ▶ line.
 */
const cardLogo = { className: "mb-2 max-h-12 max-w-3/5", sizes: "264px" };

/**
 * A Continue Watching card's title over its art, unless the art carries it, and under it ▶ and its
 * `detail`: once started, a bar of how far in you are beside what's left.
 */
export function ContinueWatchingCaption({
  title,
  logoUrl,
  titled,
  detail,
  state,
}: {
  title: string;
  logoUrl: string | undefined;
  /** Whether the art carries the title, which then isn't drawn over it. */
  titled: boolean;
  detail: string | undefined;
  state: WatchState | undefined;
}) {
  return (
    <>
      {!titled && (
        <span className="truncate text-tile font-semibold">
          <TitleOrLogo title={title} logoUrl={logoUrl} logo={cardLogo} />
        </span>
      )}
      <PlayProgress state={state} detail={detail} />
    </>
  );
}
