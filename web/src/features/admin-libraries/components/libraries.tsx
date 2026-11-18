import { CreateLibrary } from "@/features/admin-libraries/components/create-library";
import { LibraryRow } from "@/features/admin-libraries/components/library-row";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";
import { useLibraries } from "@/shared/hooks/use-libraries";

/** The Libraries page: each Library with its scan, and a form to create one. */
export function Libraries() {
  const libraries = useLibraries();
  return (
    <Page heading="Libraries">
      {libraries.length === 0 ? (
        <p className="mt-4 text-muted-foreground">No Libraries yet. Create one below.</p>
      ) : (
        <AdminList>
          {libraries.map((library) => (
            <LibraryRow key={library.id} library={library} />
          ))}
        </AdminList>
      )}
      <section aria-labelledby="create-library" className="mt-12 max-w-sm">
        <h2 id="create-library" className="font-medium">
          Create a Library
        </h2>
        <CreateLibrary />
      </section>
    </Page>
  );
}
