import { cn } from "cn";
import type { ComponentProps } from "react";

// shadcn's button group, trimmed to a row: its buttons join into one pill, such as a split Request
// button, with a hairline of their ink between them. By type, not by child: an open menu puts its
// focus guards after its button.

function ButtonGroup({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="button-group"
      className={cn(
        "flex w-fit items-stretch *:not-first-of-type:rounded-s-none *:not-first-of-type:border-s-current/20 *:not-last-of-type:rounded-e-none *:focus-visible:relative *:focus-visible:z-10",
        className,
      )}
      {...props}
    />
  );
}

export { ButtonGroup };
