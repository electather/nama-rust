/**
 * How many results Settings' search found, said to assistive technology as it changes: nothing
 * while there's no `count`, before anything is typed.
 */
export function ResultCount({ count }: { count: number | undefined }) {
  return (
    <output className="sr-only">
      {count !== undefined && `${count} ${count === 1 ? "result" : "results"}`}
    </output>
  );
}
