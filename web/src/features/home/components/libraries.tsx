import { LibraryLink } from "@/features/home/components/library-link";
import { BackdropGrid } from "@/shared/components/backdrop-grid";
import { useLibraries } from "@/shared/hooks/use-libraries";

/** The Server's Libraries as Backdrop cards, or a line saying it has none yet. */
export function Libraries() {
  const libraries = useLibraries();
  if (libraries.length === 0) {
    return (
      <p className="mt-2 text-muted-foreground">
        This Server has no Libraries yet. Once one is added, it shows up here.
      </p>
    );
  }
  return (
    <BackdropGrid>
      {libraries.map((library) => (
        <LibraryLink key={String(library.id)} library={library} />
      ))}
    </BackdropGrid>
  );
}
