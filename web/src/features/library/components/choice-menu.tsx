import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

/** A "`label`: …" button whose menu offers `choices`, each a value and what it shows. */
export function ChoiceMenu<T extends string>({
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
  const items = Object.entries<string>(choices).map(([value, name]) => (
    <DropdownMenuRadioItem key={value} value={value}>
      {name}
    </DropdownMenuRadioItem>
  ));
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
          {items}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
