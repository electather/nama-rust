/** What an empty `folder`'s warning says. */
export function EmptyFolderNote({ folder }: { folder: string }) {
  return (
    <span>
      <span className="font-medium wrap-anywhere">{folder}</span> looks empty. Is the drive
      connected? Nothing was removed.
    </span>
  );
}
