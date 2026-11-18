import { BackdropCard } from "@/features/home/components/backdrop-card";
import type { Library } from "@/gen/nama/v1/library_pb";
import { kindName } from "@/shared/lib/library-kind";

/** A Library's Backdrop with its name over it and its kind under it. */
export function LibraryCard({ library }: { library: Library }) {
  return (
    <BackdropCard
      url={library.backdropUrl}
      thumbhash={library.backdropThumbhash}
      title={library.name}
      detail={kindName(library.kind)}
    />
  );
}
