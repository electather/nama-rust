import type { ReactNode } from "react";

// Over a row's own link, where that covers the row, so what's at the end is still pressed itself.
const END = "relative flex flex-wrap items-center justify-end gap-2.5 empty:hidden";

const LOOKS = {
  plain: { row: "", lines: "" },
  // A row that's a link lights under the pointer, a little past its ends, and the hairlines over
  // and under it go, as they'd cut across its corners.
  linked: {
    row: "-mx-2 rounded-lg px-2 transition-colors hover:bg-muted/50 active:bg-muted motion-reduce:transition-none",
    lines: "group-hover:border-transparent [li:hover+li_&]:border-transparent",
  },
};

/**
 * One thing on a pane's list, such as a Device: what `start` draws, its `name` over a line about
 * it and why the last change to it failed, and at the end its state and buttons, if it has any,
 * which wrap under it when narrow. The hairline between two starts at the name, past what `start` draws. Its list
 * sets no gap, and pulls its ends in by the 10px each row keeps over and under itself.
 */
export function ItemRow({
  start,
  name,
  meta,
  failed,
  beside = false,
  linked = false,
  children,
}: {
  start: ReactNode;
  name: ReactNode;
  meta: ReactNode;
  failed?: string | undefined;
  /** Keeps what's at the end beside the name on a phone while it fits, as a lone "…" does. */
  beside?: boolean;
  /** `name` is a link stretched over the row (`after:absolute after:inset-0`), so all of it opens. */
  linked?: boolean;
  children?: ReactNode;
}) {
  const look = LOOKS[linked ? "linked" : "plain"];
  const words = (
    <div className="flex min-w-0 flex-1 basis-50 flex-col gap-0.5">
      <span className="truncate text-sm font-medium">{name}</span>
      <span className="text-xs text-pretty text-muted-foreground">{meta}</span>
      <span role="alert" className="text-xs text-pretty text-destructive empty:hidden">
        {failed}
      </span>
    </div>
  );
  return (
    <li className={`group relative flex gap-x-3 ${look.row}`}>
      <span className="flex shrink-0 py-2.5">{start}</span>
      <div
        className={`flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-2 py-2.5 group-not-first:border-t ${look.lines}`}
      >
        {words}
        <div className={beside ? `${END} ms-auto` : `${END} max-md:basis-full`}>{children}</div>
      </div>
    </li>
  );
}
