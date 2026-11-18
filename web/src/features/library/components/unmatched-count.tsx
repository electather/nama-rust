/** How many Items it lists, once the first page has said. */
export function UnmatchedCount({ count }: { count: number | undefined }) {
  if (count === undefined) {
    return null;
  }
  return (
    <span className="ms-2 text-sm font-normal text-muted-foreground">{count.toLocaleString()}</span>
  );
}
