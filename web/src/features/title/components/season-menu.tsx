import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { seasonName } from "@/shared/lib/title-facts";

/** The Season the row of Episodes is on, `current`, as a button whose menu scrolls it to another. */
export function SeasonMenu({
  seasons,
  current,
  onPick,
}: {
  seasons: number[];
  current: number;
  onPick: (season: number) => void;
}) {
  const trigger = (
    <Button variant="secondary" size="heading">
      {seasonName(current)}
      <ChevronDown aria-hidden />
    </Button>
  );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={trigger} />
      <DropdownMenuContent align="start">
        {seasons.map((number) => (
          <DropdownMenuItem
            key={number}
            onClick={() => {
              onPick(number);
            }}
          >
            {seasonName(number)}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
