/** What a Library with no titles says, until the Server has scanned and Matched its folders. */
export function EmptyLibrary({ titles }: { titles: string }) {
  return (
    <div className="mt-8">
      <p className="font-medium">Nothing in this Library yet</p>
      <p className="mt-1 max-w-prose text-muted-foreground">
        Its {titles} show up here once the Server has scanned its folders and Matched what they
        hold.
      </p>
    </div>
  );
}
