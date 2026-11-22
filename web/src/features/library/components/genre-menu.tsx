import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { chosenGenres } from "@/features/library/lib/chosen-genres";
import type { BarProps } from "@/features/library/lib/listing-bar";

/**
 * A "Genre: …" button whose menu turns each of `genres` on or off, keeping titles with any genre
 * that's on, and turns them all off with "All".
 */
export function GenreMenu({ listing, change, genres }: BarProps & { genres: string[] }) {
  const chosen = listing.genres ?? [];
  const choose = (changed: string[]) => {
    change({ ...listing, genres: changed.length === 0 ? undefined : changed });
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" />}>
        Genre: {chosenGenres(chosen)}
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuCheckboxItem
          checked={chosen.length === 0}
          onCheckedChange={() => {
            choose([]);
          }}
        >
          All
        </DropdownMenuCheckboxItem>
        {genres.map((genre) => (
          <DropdownMenuCheckboxItem
            key={genre}
            checked={chosen.includes(genre)}
            onCheckedChange={(checked) => {
              choose(checked ? [...chosen, genre] : chosen.filter((named) => named !== genre));
            }}
          >
            {genre}
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
