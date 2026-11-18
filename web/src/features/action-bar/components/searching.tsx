import { LoaderCircle } from "lucide-react";

/** Says a search is loading, over what the last one found. */
export function Searching() {
  return (
    <output className="flex items-center gap-2 p-2 text-xs text-muted-foreground">
      <LoaderCircle
        aria-hidden
        strokeWidth={1.5}
        className="size-3.5 animate-spin motion-reduce:animate-none"
      />
      Searching…
    </output>
  );
}
