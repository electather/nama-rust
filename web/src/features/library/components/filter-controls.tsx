import { ChoiceMenu } from "@/features/library/components/choice-menu";
import { GenreMenu } from "@/features/library/components/genre-menu";
import { YearInput } from "@/features/library/components/year-input";
import { ANY, type BarProps } from "@/features/library/lib/listing-bar";
import { type Watched, watchedStates } from "@/features/library/lib/title-listing";

/** The genre and watch state menus and the year range. */
export function FilterControls({ listing, change, genres }: BarProps & { genres: string[] }) {
  return (
    <>
      <GenreMenu genres={genres} listing={listing} change={change} />
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
