import { Button } from "nama-ui/components/button";

import type { BarProps } from "@/features/library/lib/listing-bar";

/** A chip for each of `genres`, which turns it on or off, keeping titles with any that's on. */
export function GenreChips({ listing, change, genres }: BarProps & { genres: string[] }) {
  const chosen = listing.genres ?? [];
  return (
    <fieldset className="flex flex-wrap gap-2">
      <legend className="mb-2.5 text-sm font-medium">Genre</legend>
      {genres.map((genre) => {
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
    </fieldset>
  );
}
