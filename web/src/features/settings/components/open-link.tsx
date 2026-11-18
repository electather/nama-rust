/** The link to finish connecting in the other app, in a new tab. */
export function OpenLink({ href, name }: { href: string; name: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="rounded-lg px-4 py-2 text-sm font-medium underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      Open {name}
    </a>
  );
}
