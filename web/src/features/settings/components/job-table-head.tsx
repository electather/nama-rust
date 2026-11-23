const CELL = "px-2 pb-1.5 text-start text-xs font-normal text-muted-foreground";

/** The names of the Jobs table's columns; Next run is left out on a phone, as its cells are. */
export function JobTableHead() {
  return (
    <thead>
      <tr>
        <th className={CELL}>Job</th>
        <th className={`${CELL} w-28`}>Last run</th>
        <th className={`${CELL} w-36 max-md:hidden`}>Next run</th>
        {/* Off-screen, so its column is as wide as Run now itself. */}
        <th className="sr-only">Run now</th>
      </tr>
    </thead>
  );
}
