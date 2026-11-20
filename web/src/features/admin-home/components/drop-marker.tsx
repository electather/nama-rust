import { cn } from "cn";

/** The line on a row's `edge` where the row being dragged would drop. */
export function DropMarker({ edge }: { edge: "top" | "bottom" }) {
  return (
    <span
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-x-3 h-0.5 rounded-full bg-foreground",
        edge === "top" ? "-top-px" : "-bottom-px",
      )}
    />
  );
}
