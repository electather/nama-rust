import { Search } from "lucide-react";

/** The field that narrows the sidebar to the sections answering to what's `typed`. Not on a phone. */
export function SettingsSearch({
  typed,
  onType,
}: {
  typed: string;
  onType: (typed: string) => void;
}) {
  return (
    // DESIGN.md, "Inputs", drawn around the icon and the field together.
    <label className="flex h-8 shrink-0 items-center gap-2 rounded-lg border border-input bg-input/30 px-2 text-muted-foreground transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 max-md:hidden">
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <input
        type="search"
        aria-label="Search settings"
        placeholder="Search settings"
        // Opening Settings starts here, so typing finds a section at once.
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- a modal dialog puts focus on the control its design names, which this is
        autoFocus
        value={typed}
        onChange={(event) => {
          onType(event.target.value);
        }}
        className="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
      />
    </label>
  );
}
