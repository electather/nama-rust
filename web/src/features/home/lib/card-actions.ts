import type { Card } from "@/features/home/lib/continue-watching-cards";

/** What a Continue Watching card's menu offers. */
export interface CardActions {
  page: Card["page"];
  onMarkWatched: () => void;
  onRemove: () => void;
}
