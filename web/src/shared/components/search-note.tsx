import { cn } from "cn";

/**
 * The quiet note under a search's results saying what is missing from them, `searchNote`'s, set
 * where its screen puts it by `className`. Always there, and never hidden, so screen readers
 * announce it as it fills.
 */
export function SearchNote({ note, className }: { note: string | undefined; className: string }) {
  return (
    <output aria-live="polite" className={cn("text-muted-foreground", className)}>
      {note}
    </output>
  );
}
