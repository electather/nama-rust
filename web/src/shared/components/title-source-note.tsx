import { cn } from "cn";

/**
 * The quiet note under a search's results when the titles to request couldn’t be searched, set
 * where its screen puts it by `className`. Always there, and never hidden, so screen readers
 * announce it as it fills.
 */
export function TitleSourceNote({ shown, className }: { shown: boolean; className: string }) {
  return (
    <output aria-live="polite" className={cn("text-muted-foreground", className)}>
      {shown ? "Titles to request couldn’t be searched, so only this Server’s are listed." : ""}
    </output>
  );
}
