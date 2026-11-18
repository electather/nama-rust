/**
 * While the Libraries load, or if they `failed` to, says so and sends the `held` ids as they were:
 * the Server keeps a left-out value only for a Secret, so sending none would clear them.
 */
export function UnlistedLibraries({
  name,
  held,
  failed,
}: {
  name: string;
  held: bigint[];
  failed: boolean;
}) {
  return (
    <>
      <p className="text-sm text-muted-foreground">
        {failed
          ? "The Libraries couldn’t be listed, so saving keeps the ones picked."
          : "Loading the Libraries…"}
      </p>
      {held.map((id) => (
        <input key={String(id)} type="hidden" name={name} value={String(id)} />
      ))}
    </>
  );
}
