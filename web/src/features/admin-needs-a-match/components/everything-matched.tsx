/** Says everything is Matched, unless a Library listed above needs a Match (`anyListed`). */
export function EverythingMatched({ anyListed }: { anyListed: boolean }) {
  return anyListed ? undefined : <p className="mt-10 font-medium">Everything is Matched.</p>;
}
