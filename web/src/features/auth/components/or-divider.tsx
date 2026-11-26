/** "Or" between a sign-in form and the other ways in under it. */
export function OrDivider() {
  return (
    <div className="flex items-center gap-3.5 text-sm text-muted-foreground">
      <span aria-hidden className="h-px flex-1 bg-border" />
      Or
      <span aria-hidden className="h-px flex-1 bg-border" />
    </div>
  );
}
