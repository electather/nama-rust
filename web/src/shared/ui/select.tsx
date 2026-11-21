import { Select as SelectPrimitive } from "@base-ui/react/select";
import { cn } from "cn";
import { Check, ChevronDown } from "lucide-react";

// shadcn's base-nova select, trimmed to the parts we use. DESIGN.md, "Inputs" for the field and
// "Menus" for its list, which opens under the field as a menu does, never over it.

/** Give it `items`, each a `value` and `label`, so the field can name the chosen one while shut. */
const Select = SelectPrimitive.Root;

/** The field: the chosen item's words, cut short if they don't fit, and a chevron. 44px on a phone. */
function SelectTrigger({ className, ...props }: SelectPrimitive.Trigger.Props) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-trigger"
      className={cn(
        "flex h-11 w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-lg border border-input bg-input/30 ps-2.5 pe-2 text-sm transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-muted aria-invalid:border-destructive/50 md:h-8",
        className,
      )}
      {...props}
    >
      <SelectPrimitive.Value
        data-slot="select-value"
        className="min-w-0 flex-1 truncate text-start"
      />
      <SelectPrimitive.Icon className="shrink-0 text-muted-foreground">
        <ChevronDown strokeWidth={1.5} className="size-4" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

/**
 * The list, at least as wide as the field. Inside a modal `<dialog>`, give it that dialog as
 * `container`: the page under one can't be seen or clicked, the list included.
 */
function SelectContent({
  align = "start",
  sideOffset = 4,
  container,
  className,
  ...props
}: SelectPrimitive.Popup.Props &
  Pick<SelectPrimitive.Positioner.Props, "align" | "sideOffset"> &
  Pick<SelectPrimitive.Portal.Props, "container">) {
  return (
    <SelectPrimitive.Portal container={container}>
      <SelectPrimitive.Positioner
        className="isolate z-50 outline-none"
        align={align}
        sideOffset={sideOffset}
        alignItemWithTrigger={false}
      >
        <SelectPrimitive.Popup
          data-slot="select-content"
          className={cn(
            "z-50 max-h-(--available-height) min-w-(--anchor-width) origin-(--transform-origin) overflow-y-auto overscroll-contain rounded-lg border bg-popover p-1 text-popover-foreground shadow-md duration-150 ease-out-expo outline-none motion-reduce:animate-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0",
            className,
          )}
          {...props}
        />
      </SelectPrimitive.Positioner>
    </SelectPrimitive.Portal>
  );
}

/** One choice, with a check where the chosen one is. 44px tall under a finger, as a menu's items. */
function SelectItem({ className, children, ...props }: SelectPrimitive.Item.Props) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(
        "flex min-h-11 cursor-default items-center gap-2.5 rounded-md ps-2 pe-2.5 text-sm outline-hidden select-none data-highlighted:bg-accent pointer-fine:min-h-8 data-disabled:pointer-events-none data-disabled:text-muted-foreground",
        className,
      )}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <Check aria-hidden className="size-4" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <SelectPrimitive.ItemText className="whitespace-nowrap">{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  );
}

export { Select, SelectContent, SelectItem, SelectTrigger };
