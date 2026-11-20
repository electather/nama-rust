import { Link } from "@tanstack/react-router";

import { BackdropCard } from "@/features/home/components/backdrop-card";
import { CardMenu } from "@/features/home/components/card-menu";
import type { Card } from "@/features/home/lib/continue-watching-cards";
import { useSetPlayed } from "@/shared/hooks/use-set-played";
import { posterLink } from "@/shared/lib/poster-classes";

/** A Continue Watching card in a grid, opening where it plays, with its menu to mark it watched. */
export function ContinueWatchingItem({ card }: { card: Card }) {
  const setPlayed = useSetPlayed();
  return (
    <li className="relative">
      <Link {...card.link} className={posterLink}>
        <BackdropCard {...card.backdrop} />
      </Link>
      <CardMenu
        title={card.backdrop.title}
        onMarkWatched={() => {
          setPlayed(card.markWatched);
        }}
      />
    </li>
  );
}
