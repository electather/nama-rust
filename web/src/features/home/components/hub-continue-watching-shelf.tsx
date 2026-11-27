import { ContinueWatchingCard } from "@/features/home/components/continue-watching-card";
import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { useHubCardActions } from "@/features/home/data/use-hub-card-actions";
import type { useHubContinueWatching } from "@/features/home/data/use-hub-continue-watching";
import { OnServer } from "@/shared/components/on-server";
import { backdropTile } from "@/shared/lib/tile-shapes";

/**
 * The Hub's Continue Watching `cards`, each opening on its Server, its skeleton while there are
 * none and a Server is still `waiting`, loading `next` as it nears its end.
 */
export function HubContinueWatchingShelf({
  cards,
  waiting,
  complete,
  next,
}: Omit<ReturnType<typeof useHubContinueWatching>, "failed">) {
  const { markWatched, remove } = useHubCardActions();
  if (cards.length === 0) {
    return waiting ? <ShelfSkeleton tile={backdropTile} /> : null;
  }
  const tiles = cards.map((merged) => ({
    key: `${merged.server.id}/${merged.card.key}`,
    tile: (
      <OnServer on={merged}>
        <ContinueWatchingCard
          card={merged.card}
          actions={{
            page: merged.card.page,
            onMarkWatched: () => {
              markWatched(merged);
            },
            onRemove: () => {
              remove(merged);
            },
          }}
        />
      </OnServer>
    ),
  }));
  return (
    <VirtualShelf
      tiles={tiles}
      tile={backdropTile}
      // ARIA's -1 is "unknown": a Server may yet add cards.
      setSize={complete ? tiles.length : -1}
      restoreId="continue-watching"
      next={next}
    />
  );
}
