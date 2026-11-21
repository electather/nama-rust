/** A path or command as the Admin types it, in the one typeface (DESIGN.md). */
export function CodeText({ children }: { children: string }) {
  return <code className="rounded-sm bg-muted px-1 font-sans text-sm break-all">{children}</code>;
}
