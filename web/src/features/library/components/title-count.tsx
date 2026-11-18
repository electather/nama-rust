/** "1 Movie", "1,000 Movies": how many titles the listing has, once the first page has said. */
export function TitleCount({
  count,
  titles,
}: {
  count: number | undefined;
  titles: { one: string; many: string };
}) {
  if (count === undefined) {
    return null;
  }
  return (
    <p className="mt-1 text-sm text-muted-foreground">
      {count.toLocaleString()} {count === 1 ? titles.one : titles.many}
    </p>
  );
}
