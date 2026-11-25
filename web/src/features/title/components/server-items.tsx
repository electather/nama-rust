import {
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from "nama-ui/components/dropdown-menu";

import { ServerRadios } from "@/features/title/components/server-radios";
import type { PlayFrom } from "@/features/title/lib/server-choices";

/**
 * The choice of which Server Play plays from, atop a title's menu at the Hub. Nothing while only
 * one has the title.
 */
export function ServerItems(playFrom: PlayFrom) {
  if (playFrom.choices.length < 2) {
    return null;
  }
  return (
    <>
      <DropdownMenuGroup>
        <DropdownMenuLabel>Server</DropdownMenuLabel>
        <ServerRadios {...playFrom} />
      </DropdownMenuGroup>
      <DropdownMenuSeparator />
    </>
  );
}
