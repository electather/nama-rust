/** Says nothing matches what was `typed`, or, before anything is, how to search. */
export function NoResults({ typed }: { typed: string }) {
  return (
    <div className="flex flex-col items-center gap-1.5 px-4 py-10 text-center text-sm text-muted-foreground">
      {typed === "" ? "Type a title to search your Libraries." : `No results for “${typed}”`}
      {typed !== "" && <span className="text-xs">Try a different title, page or action.</span>}
    </div>
  );
}
