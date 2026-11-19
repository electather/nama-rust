import { LoaderCircle, Search } from "lucide-react";

/** The field's leading icon, which turns into a spinner once a search has kept you waiting. */
export function SearchMark({ slow }: { slow: boolean }) {
  const Icon = slow ? LoaderCircle : Search;
  return (
    <>
      <Icon
        aria-hidden
        strokeWidth={1.5}
        className="size-5 shrink-0 text-muted-foreground data-[slow=true]:animate-spin motion-reduce:animate-none"
        data-slow={slow}
      />
      <output aria-live="polite" className="sr-only">
        {slow ? "Searching…" : ""}
      </output>
    </>
  );
}
