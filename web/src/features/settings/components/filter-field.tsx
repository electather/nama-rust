import { Search } from "lucide-react";

/** A field over a list that narrows it to what answers to the words `typed`, named `label`. */
export function FilterField({
  label,
  typed,
  onType,
}: {
  label: string;
  typed: string;
  onType: (typed: string) => void;
}) {
  return (
    // DESIGN.md, "Inputs", drawn around the icon and the field together.
    <label className="flex h-11 min-w-0 flex-1 basis-50 items-center gap-2 rounded-lg border border-input bg-input/30 px-2.5 text-muted-foreground transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 md:h-8">
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <input
        type="search"
        aria-label={label}
        placeholder={label}
        value={typed}
        onChange={(event) => {
          onType(event.target.value);
        }}
        // 16px on a phone, where iOS zooms into a field with smaller text.
        className="min-w-0 flex-1 bg-transparent text-base text-foreground outline-none placeholder:text-muted-foreground md:text-sm"
      />
    </label>
  );
}
