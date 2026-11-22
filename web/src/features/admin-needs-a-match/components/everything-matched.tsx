/** Says everything is Matched, unless a Library listed above needs a Match (`anyListed`). */
export function EverythingMatched({ anyListed }: { anyListed: boolean }) {
  return anyListed ? undefined : (
    <p className="text-sm text-muted-foreground">Everything is Matched.</p>
  );
}
