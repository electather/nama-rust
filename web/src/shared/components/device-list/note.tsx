/** A line of muted text where the list of Devices would be. */
export function Note({ children }: { children: string }) {
  return <p className="text-sm text-muted-foreground">{children}</p>;
}
