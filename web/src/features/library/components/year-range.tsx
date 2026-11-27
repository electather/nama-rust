import { YearInput } from "@/features/library/components/year-input";
import type { BarProps } from "@/features/library/lib/listing-bar";

/** The From and To years that keep titles released, or first aired, between them. */
export function YearRange({ listing, change }: BarProps) {
  return (
    <div className="flex items-center gap-2 text-muted-foreground">
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
  );
}
