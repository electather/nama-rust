import { Ellipsis } from "lucide-react";

import { Button } from "@/shared/ui/button";
import { DropdownMenuTrigger } from "@/shared/ui/dropdown-menu";

/** The button of a card's menu, named for the title it's over. */
export function HideMenuTrigger({ title }: { title: string }) {
  return (
    <DropdownMenuTrigger
      render={<Button variant="overlay" size="icon" aria-label={`More for ${title}`} />}
    >
      <Ellipsis aria-hidden />
    </DropdownMenuTrigger>
  );
}
