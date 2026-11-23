import { LibraryItem } from "@/features/settings/components/library-item";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import type { Library } from "@/gen/nama/v1/library_pb";

/** Every Library on the Server, each opening in place of the list. */
export function LibraryList({ libraries }: { libraries: Library[] }) {
  return (
    <PaneFrame section="libraries">
      {libraries.length === 0 ? (
        <p className="py-3.5 text-sm text-muted-foreground">No Libraries yet.</p>
      ) : (
        <ul aria-label="Libraries" className="-my-2.5 flex flex-col">
          {libraries.map((library) => (
            <LibraryItem key={library.id} library={library} />
          ))}
        </ul>
      )}
    </PaneFrame>
  );
}
