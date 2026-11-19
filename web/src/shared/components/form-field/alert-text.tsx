/**
 * Why the Server refused, in red. Always there, empty until there's an `error`, so screen readers
 * announce one as it appears.
 */
export function AlertText({ id, error }: { id?: string; error: string | undefined }) {
  return (
    <p id={id} role="alert" className="order-4 basis-full text-sm text-destructive empty:hidden">
      {error}
    </p>
  );
}
