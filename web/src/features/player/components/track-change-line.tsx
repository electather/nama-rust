/** A Track as it plays, after the file's own when converting changed it. */
export function TrackChangeLine({ from, to }: { from: string | undefined; to: string }) {
  return (
    <>
      {from !== undefined && (
        <>
          {from} <span className="text-muted-foreground">→</span>{" "}
        </>
      )}
      {to}
    </>
  );
}
