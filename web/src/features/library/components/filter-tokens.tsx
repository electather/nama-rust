import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";

import type { BarProps } from "@/features/library/lib/listing-bar";
import { filterTokens, unfiltered } from "@/features/library/lib/title-listing";

/** A chip for each filter that's on, which turns it off, then Clear all. Nothing if none is. */
export function FilterTokens({ listing, change }: BarProps) {
  const tokens = filterTokens(listing);
  if (tokens.length === 0) {
    return null;
  }
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {tokens.map(({ name, without }) => (
        <Button
          key={name}
          variant="secondary"
          aria-label={`Remove ${name}`}
          onClick={() => {
            change(without);
          }}
        >
          {name}
          <X aria-hidden />
        </Button>
      ))}
      <Button
        variant="ghost"
        onClick={() => {
          change(unfiltered(listing));
        }}
      >
        Clear all
      </Button>
    </div>
  );
}
