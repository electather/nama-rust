import { X } from "lucide-react";

import { RowIcon } from "@/features/action-bar/components/row-icon";

/** The kind a search is narrowed to, in the lit state, clearing it when pressed. */
export function ScopeChip({ kind, onClear }: { kind: "movies" | "shows"; onClear: () => void }) {
  return (
    <button
      type="button"
      aria-label={`Clear ${kind === "movies" ? "Movie" : "Show"} filter`}
      title="Backspace clears the filter"
      onClick={onClear}
      className="inline-flex h-6 shrink-0 cursor-pointer items-center gap-1.5 rounded-full border border-primary-dim bg-primary-tint px-2 text-xs font-medium text-primary transition-colors duration-150 ease-out-expo outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-popover motion-reduce:transition-none"
    >
      <RowIcon name={kind === "movies" ? "film" : "tv"} />
      {kind === "movies" ? "Movie" : "Show"}
      <X aria-hidden strokeWidth={1.5} className="size-3 opacity-70" />
    </button>
  );
}
