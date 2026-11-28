import type { ReactNode } from "react";

/**
 * The top of Needs a Match's list: its `title` (`titleId` names the pane by it) as a large headline,
 * saying how many files there are (`total`) and what to do about them, and `children` under it,
 * which picks the Library.
 */
export function NeedsAMatchHeading({
  titleId,
  title,
  total,
  children,
}: {
  titleId: string;
  title: string;
  total: number;
  children: ReactNode;
}) {
  return (
    <header className="flex flex-col gap-3.5">
      <div className="flex flex-col gap-1">
        <h3 id={titleId} className="text-3xl font-medium tracking-tight">
          {title}
        </h3>
        <p className="text-sm text-pretty text-muted-foreground empty:hidden">
          {total > 0 &&
            `The Server couldn’t tell what ${total === 1 ? "this file is" : `these ${total} files are`}. Open one to pick its title.`}
        </p>
      </div>
      {children}
    </header>
  );
}
