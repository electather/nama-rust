/** Says the Server will refuse the delete because it can't write to `folder`, if there is one. */
export function RefusedFor({ folder }: { folder: string | undefined }) {
  if (folder === undefined) {
    return null;
  }
  return (
    <p className="text-destructive">
      The Server will refuse to delete them: it can’t write to {folder}.
    </p>
  );
}
