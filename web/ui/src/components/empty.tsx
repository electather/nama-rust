import { cn } from "cn";
import type { ComponentProps } from "react";

// shadcn's base-nova empty, trimmed to the parts we use and restyled to DESIGN.md: an icon on a
// Muted tile with a hairline, as a Settings row's, over a title in Interface.

/** Where a list or pane says it has nothing to show, centred in the space it has. */
function Empty({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="empty"
      className={cn(
        "flex w-full min-w-0 flex-1 flex-col items-center justify-center gap-4 p-6 text-center text-balance",
        className,
      )}
      {...props}
    />
  );
}

/** The empty state's icon and words, stacked. */
function EmptyHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="empty-header"
      className={cn("flex max-w-sm flex-col items-center gap-2", className)}
      {...props}
    />
  );
}

/** The empty state's icon, on a Muted tile with a hairline. */
function EmptyMedia({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="empty-icon"
      className={cn(
        "mb-2 flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground inset-ring inset-ring-border [&_svg]:pointer-events-none [&_svg]:size-5 [&_svg]:shrink-0",
        className,
      )}
      {...props}
    />
  );
}

/** What the empty state says, in Interface medium. */
function EmptyTitle({ className, ...props }: ComponentProps<"div">) {
  return (
    <div data-slot="empty-title" className={cn("text-sm font-medium", className)} {...props} />
  );
}

export { Empty, EmptyHeader, EmptyMedia, EmptyTitle };
