import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  type Sort,
  type TitleListing,
  type Watched,
  isDescending,
  sorts,
  watchedStates,
} from "@/lib/title-listing";

/** What a choice menu shows for no choice, which lists every title. */
const ANY = "";

/** What a part of the bar changes: the listing, and what gets the listing changed. */
export interface BarProps {
  listing: TitleListing;
  change: (changed: TitleListing) => void;
}

/**
 * The sort and filter bar over a Library's grid: sort, its direction, a genre of `genres`, a year
 * range and watched state. `change` gets the listing with what the User changed.
 */
export function TitleListControls({ genres, ...bar }: BarProps & { genres: string[] }) {
  return (
    <div className="mt-6 flex flex-wrap items-center gap-x-1 gap-y-2 -ms-4">
      <SortControls {...bar} />
      <FilterControls genres={genres} {...bar} />
    </div>
  );
}

function SortControls({ listing, change }: BarProps) {
  const descending = isDescending(listing);
  return (
    <>
      <ChoiceMenu
        label="Sort"
        choices={sorts}
        chosen={listing.sort ?? "title"}
        choose={(sort: Sort) => {
          // A new sort runs its own way, so the direction from the last one goes.
          change({ ...listing, sort, order: undefined });
        }}
      />
      <Button
        variant="ghost"
        size="icon"
        aria-label={descending ? "Sort ascending" : "Sort descending"}
        onClick={() => {
          change({ ...listing, order: descending ? "asc" : "desc" });
        }}
      >
        {descending ? <ArrowDownWideNarrow aria-hidden /> : <ArrowUpNarrowWide aria-hidden />}
      </Button>
    </>
  );
}

function FilterControls({ listing, change, genres }: BarProps & { genres: string[] }) {
  return (
    <>
      <ChoiceMenu
        label="Genre"
        choices={{ [ANY]: "All", ...Object.fromEntries(genres.map((genre) => [genre, genre])) }}
        chosen={listing.genre ?? ANY}
        choose={(genre: string) => {
          change({ ...listing, genre: genre === ANY ? undefined : genre });
        }}
      />
      <ChoiceMenu
        label="Watch State"
        choices={{ [ANY]: "All", ...watchedStates }}
        chosen={listing.watched ?? ANY}
        choose={(watched: Watched | typeof ANY) => {
          change({ ...listing, watched: watched === ANY ? undefined : watched });
        }}
      />
      <div className="ms-4 flex items-center gap-2 text-sm text-muted-foreground">
        <YearInput
          label="From year"
          placeholder="From"
          year={listing.from}
          change={(from) => {
            change({ ...listing, from });
          }}
        />
        <span aria-hidden>–</span>
        <YearInput
          label="To year"
          placeholder="To"
          year={listing.to}
          change={(to) => {
            change({ ...listing, to });
          }}
        />
      </div>
    </>
  );
}

/** A "`label`: …" button whose menu offers `choices`, each a value and what it shows. */
function ChoiceMenu<T extends string>({
  label,
  choices,
  chosen,
  choose,
}: {
  label: string;
  choices: Record<T, string>;
  chosen: T;
  choose: (choice: T) => void;
}) {
  const entries = Object.entries<string>(choices);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" />}>
        {label}: {choices[chosen]}
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuRadioGroup
          value={chosen}
          onValueChange={(value: T) => {
            choose(value);
          }}
        >
          {entries.map(([value, name]) => (
            <DropdownMenuRadioItem key={value} value={value}>
              {name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** A year field that changes the listing once it holds a four-digit year, or is cleared. */
function YearInput({
  label,
  placeholder,
  year,
  change,
}: {
  label: string;
  placeholder: string;
  year: number | undefined;
  change: (year?: number) => void;
}) {
  const [typed, setTyped] = useState(year === undefined ? "" : String(year));
  return (
    <div className="w-24">
      <Input
        type="number"
        inputMode="numeric"
        aria-label={label}
        placeholder={placeholder}
        value={typed}
        onChange={(event) => {
          const text = event.target.value;
          setTyped(text);
          if (text === "") change();
          else if (/^\d{4}$/u.test(text)) change(Number(text));
        }}
      />
    </div>
  );
}
