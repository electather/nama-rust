import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import type { BarProps } from "@/features/library/lib/listing-bar";
import { type Sort, isDescending, orderNames, sorts } from "@/features/library/lib/title-listing";

/** The "Sort: …" button, whose menu picks the sort and then, in words, which way it runs. */
export function SortMenu({ listing, change }: BarProps) {
  const sort = listing.sort ?? "title";
  const orders = orderNames(sort);
  const sortItems = Object.entries<string>(sorts).map(([value, name]) => (
    <DropdownMenuRadioItem key={value} value={value}>
      {name}
    </DropdownMenuRadioItem>
  ));
  const bySort = (
    <DropdownMenuRadioGroup
      value={sort}
      onValueChange={(chosen: Sort) => {
        // A new sort runs its own way, so the direction from the last one goes.
        change({ ...listing, sort: chosen, order: undefined });
      }}
    >
      {sortItems}
    </DropdownMenuRadioGroup>
  );
  const byOrder = (
    <DropdownMenuRadioGroup
      value={isDescending(listing) ? "desc" : "asc"}
      onValueChange={(order: "asc" | "desc") => {
        change({ ...listing, order });
      }}
    >
      <DropdownMenuRadioItem value="asc">{orders.asc}</DropdownMenuRadioItem>
      <DropdownMenuRadioItem value="desc">{orders.desc}</DropdownMenuRadioItem>
    </DropdownMenuRadioGroup>
  );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" />}>
        Sort: {sorts[sort]}
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        {bySort}
        <DropdownMenuSeparator />
        {byOrder}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
