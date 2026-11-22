/** The files of a Library whose names don't say which Movie or Episode they are, if it has any. */
export function FilesToRename({ paths }: { paths: string[] }) {
  if (paths.length === 0) {
    return null;
  }
  return (
    <div className="mt-4">
      <h5 className="text-sm font-medium text-muted-foreground">Files to rename</h5>
      <p className="mt-1 text-xs text-pretty text-muted-foreground">
        Their names don’t say which Movie or Episode they are. Rename them, then scan the Library.
      </p>
      <ul className="mt-2 flex flex-col gap-1 text-xs wrap-anywhere">
        {paths.map((path) => (
          <li key={path}>{path}</li>
        ))}
      </ul>
    </div>
  );
}
