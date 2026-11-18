// DESIGN.md "State labels": a hairline pill, so its edge stays on the current link's pill.
/** How many Requests are Pending, beside the link to them. */
export function PendingBadge({ pending }: { pending: number }) {
  return (
    <span className="ms-1.5 rounded-full border px-1.5 text-xs text-foreground tabular-nums">
      {pending > 99 ? "99+" : pending}
    </span>
  );
}
