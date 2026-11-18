/** The files of a Library whose names don't say which Movie or Episode they are. */
export function FilesToRename({ paths }: { paths: string[] }) {
  return (
    <div className="mt-6">
      <h3 className="text-sm font-medium">Files to rename</h3>
      <p className="mt-1 text-sm text-pretty text-muted-foreground">
        Their names don’t say which Movie or Episode they are. Rename them, then scan the Library.
      </p>
      <ul className="mt-2 flex flex-col gap-1 text-sm wrap-anywhere">
        {paths.map((path) => (
          <li key={path}>{path}</li>
        ))}
      </ul>
    </div>
  );
}
