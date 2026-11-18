import { useDeleteLibrary } from "@/features/admin-libraries/data/use-delete-library";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import type { Library } from "@/gen/nama/v1/library_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";

/** The button that deletes `library`, once the Admin confirms, then goes back to the Libraries. */
export function DeleteLibrary({ library }: { library: Library }) {
  const remove = useDeleteLibrary();
  return (
    <ConfirmDialog
      action="Delete"
      name={`Delete ${library.name}`}
      title={`Delete ${library.name}?`}
      description="Its Movies or Shows leave the Server. The files in its folders stay where they are. This can’t be undone."
      onConfirm={async () => {
        await remove.mutateAsync({ id: library.id });
      }}
      refusal={(error) => libraryErrors(error, "delete the Library").form}
    />
  );
}
