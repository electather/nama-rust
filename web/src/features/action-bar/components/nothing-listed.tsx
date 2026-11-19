/** What the action bar says with no row to list: for `query`, or before anything is typed. */
export function NothingListed({ query }: { query: string }) {
  const [title, advice] =
    query.trim() === ""
      ? ["Nothing here yet", "Type a title to search your Libraries."]
      : [`No results for “${query}”`, "Try a different title, page or action."];
  return (
    <div className="flex flex-col items-center gap-1.5 px-4 py-10 text-center text-sm text-muted-foreground">
      <span>{title}</span>
      <span className="text-xs">{advice}</span>
    </div>
  );
}
