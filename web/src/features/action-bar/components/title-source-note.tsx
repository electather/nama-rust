/**
 * The quiet note under the results when the titles to request couldn’t be searched. Always there,
 * and never hidden, so screen readers announce it as it fills.
 */
export function TitleSourceNote({ shown }: { shown: boolean }) {
  return (
    <output aria-live="polite" className="px-3 pb-2 text-xs text-muted-foreground empty:p-0">
      {shown ? "Titles to request couldn’t be searched, so only this Server’s are listed." : ""}
    </output>
  );
}
