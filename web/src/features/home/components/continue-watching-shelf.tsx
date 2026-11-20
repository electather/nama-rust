import { ContinueWatchingCard } from "@/features/home/components/continue-watching-card";
import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { useContinueWatching } from "@/features/home/data/use-continue-watching";
import { useCardActions } from "@/features/home/hooks/use-card-actions";
import { entryCard } from "@/features/home/lib/continue-watching-cards";
import { backdropTile } from "@/features/home/lib/shelf-tiles";

/**
 * What you've started and not played, and the next Episode of each Show you're watching, most
 * recent first, each with a menu to mark it watched or remove it. Nothing at all when there's none.
 */
export function ContinueWatchingShelf({ headingId }: { headingId: string }) {
  const { data, hasNextPage, fetchNextPage, totalSize } = useContinueWatching();
  const { leaving, markWatched, remove } = useCardActions();
  const cards = data.pages
    .flatMap((page) => page.entries)
    .flatMap((entry) => entryCard(entry))
    .filter((card) => !leaving(card));
  if (cards.length === 0 && !hasNextPage) {
    return null;
  }
  const tiles = cards.map((card) => ({
    key: card.key,
    tile: (
      <ContinueWatchingCard
        card={card}
        actions={{
          page: card.page,
          onMarkWatched: () => {
            markWatched(card);
          },
          onRemove: () => {
            remove(card);
          },
        }}
      />
    ),
  }));
  return (
    <ContinueWatchingFrame headingId={headingId}>
      <VirtualShelf
        tiles={tiles}
        tile={backdropTile}
        setSize={totalSize}
        next={{ has: hasNextPage, load: fetchNextPage }}
      />
    </ContinueWatchingFrame>
  );
}
