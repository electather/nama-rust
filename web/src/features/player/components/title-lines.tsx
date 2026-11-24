/** The player's heading, and the line of facts under it. */
export function TitleLines({ title, line }: { title: string; line: string | undefined }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <h1 className="line-clamp-2 text-3xl font-medium tracking-tight text-balance">{title}</h1>
      {line !== undefined && line !== "" && <p className="truncate text-sm">{line}</p>}
    </div>
  );
}
