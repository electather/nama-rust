import { Ellipsis } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { DropdownMenuTrigger } from "nama-ui/components/dropdown-menu";

/** The button of a card's menu, named for the title it's over. */
export function CardMenuTrigger({ title }: { title: string }) {
  return (
    <DropdownMenuTrigger
      render={<Button variant="overlay" size="round" aria-label={`More for ${title}`} />}
    >
      <Ellipsis aria-hidden />
    </DropdownMenuTrigger>
  );
}
