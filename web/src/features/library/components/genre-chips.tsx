import { Button } from "nama-ui/components/button";
import { useState } from "react";

import type { BarProps } from "@/features/library/lib/listing-bar";

/** How many genres show before Show more. */
const FIRST = 5;

/**
 * A chip for each of `genres`, which turns it on or off, keeping titles with any that's on: the
 * first 5, then Show more for the rest. One that's on always shows.
 */
export function GenreChips({ listing, change, genres }: BarProps & { genres: string[] }) {
  const chosen = listing.genres ?? [];
  const [more, setMore] = useState(false);
  const shown = genres.filter((genre, at) => more || at < FIRST || chosen.includes(genre));
  return (
    <fieldset className="flex flex-wrap gap-1.5">
      <legend className="mb-2 text-sm font-medium">Genre</legend>
      {shown.map((genre) => {
        const on = chosen.includes(genre);
        const changed = on ? chosen.filter((named) => named !== genre) : [...chosen, genre];
        return (
          <Button
            key={genre}
            variant="secondary"
            aria-pressed={on}
            onClick={() => {
              change({ ...listing, genres: changed.length === 0 ? undefined : changed });
            }}
          >
            {genre}
          </Button>
        );
      })}
      {genres.length > shown.length && (
        <Button
          variant="ghost"
          onClick={() => {
            setMore(true);
          }}
        >
          Show more
        </Button>
      )}
    </fieldset>
  );
}
