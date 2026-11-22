import { Combobox as ComboboxPrimitive } from "@base-ui/react/combobox";
import { cn } from "cn";
import { Check, ChevronDown, Search, X } from "lucide-react";

// shadcn's base-nova combobox, trimmed to the two we use: a field that reads as `select.tsx`'s,
// with a search over its list, for a list too long to scroll through; and, `multiple`, a field of
// chips, one per picked item, typed into to narrow the list. DESIGN.md, "Inputs" for the fields and
// "Menus" for the list.

/** Give it `items`, each a `value` and `label`. Its `value` is one of them, whole. */
const Combobox = ComboboxPrimitive.Root;

/** The field: the chosen item's words, cut short if they don't fit, and a chevron. 44px on a phone. */
function ComboboxTrigger({ className, ...props }: ComboboxPrimitive.Trigger.Props) {
  return (
    <ComboboxPrimitive.Trigger
      data-slot="combobox-trigger"
      className={cn(
        "flex h-11 w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-lg border border-input bg-input/30 ps-2.5 pe-2 text-sm transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-muted aria-invalid:border-destructive/50 md:h-8",
        className,
      )}
      {...props}
    >
      <span data-slot="combobox-value" className="min-w-0 flex-1 truncate text-start">
        <ComboboxPrimitive.Value />
      </span>
      <ComboboxPrimitive.Icon className="shrink-0 text-muted-foreground">
        <ChevronDown strokeWidth={1.5} className="size-4" />
      </ComboboxPrimitive.Icon>
    </ComboboxPrimitive.Trigger>
  );
}

/**
 * The list under the field: a search named `search`, if the field isn't typed into itself, then
 * the items that match, each drawn by `children`, or `empty` when none does. Under chips, give it
 * their element as `anchor`. Inside a modal `<dialog>`, give it that dialog as `container`: the page under one
 * can't be seen or clicked, the list included.
 */
function ComboboxContent({
  search,
  empty,
  container,
  anchor,
  className,
  children,
  ...props
}: Omit<ComboboxPrimitive.Popup.Props, "children"> &
  Pick<ComboboxPrimitive.List.Props, "children"> &
  Pick<ComboboxPrimitive.Portal.Props, "container"> &
  Pick<ComboboxPrimitive.Positioner.Props, "anchor"> & { search?: string; empty: string }) {
  const searched = search !== undefined && (
    <div className="flex shrink-0 items-center gap-2 border-b ps-3 text-muted-foreground">
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <ComboboxPrimitive.Input
        data-slot="combobox-input"
        aria-label={search}
        placeholder={search}
        // 16px on a phone, where iOS zooms into a field with smaller text.
        className="h-11 min-w-0 flex-1 bg-transparent pe-3 text-base text-foreground outline-none placeholder:text-muted-foreground md:h-9 md:text-sm"
      />
    </div>
  );
  return (
    <ComboboxPrimitive.Portal container={container}>
      <ComboboxPrimitive.Positioner
        className="isolate z-50 outline-none"
        align="start"
        sideOffset={4}
        anchor={anchor}
      >
        <ComboboxPrimitive.Popup
          data-slot="combobox-content"
          className={cn(
            "z-50 flex max-h-(--available-height) min-w-(--anchor-width) origin-(--transform-origin) flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-md duration-150 ease-out-expo outline-none motion-reduce:animate-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0",
            className,
          )}
          {...props}
        >
          {searched}
          <ComboboxPrimitive.Empty className="px-3 py-2.5 text-sm text-muted-foreground empty:hidden">
            {empty}
          </ComboboxPrimitive.Empty>
          <ComboboxPrimitive.List
            data-slot="combobox-list"
            className="max-h-72 min-h-0 scroll-py-1 overflow-y-auto overscroll-contain p-1 data-empty:hidden"
          >
            {children}
          </ComboboxPrimitive.List>
        </ComboboxPrimitive.Popup>
      </ComboboxPrimitive.Positioner>
    </ComboboxPrimitive.Portal>
  );
}

/** One choice, with a check where the chosen one is. 44px tall under a finger, as a menu's items. */
function ComboboxItem({ className, children, ...props }: ComboboxPrimitive.Item.Props) {
  return (
    <ComboboxPrimitive.Item
      data-slot="combobox-item"
      className={cn(
        "flex min-h-11 cursor-default items-center gap-2.5 rounded-md ps-2 pe-2.5 text-sm whitespace-nowrap outline-hidden select-none data-highlighted:bg-accent pointer-fine:min-h-8 data-disabled:pointer-events-none data-disabled:text-muted-foreground",
        className,
      )}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <ComboboxPrimitive.ItemIndicator>
          <Check aria-hidden className="size-4" />
        </ComboboxPrimitive.ItemIndicator>
      </span>
      {children}
    </ComboboxPrimitive.Item>
  );
}

/**
 * A `multiple` field: a chip per picked item and the input that narrows the list, wrapping onto
 * more lines as chips fill it. Reads as `Input`: 44px on a phone, 32px from `md` up.
 */
function ComboboxChips({ className, ...props }: ComboboxPrimitive.Chips.Props) {
  return (
    <ComboboxPrimitive.Chips
      data-slot="combobox-chips"
      className={cn(
        "flex min-h-11 w-full min-w-0 cursor-text flex-wrap items-center gap-1 rounded-lg border border-input bg-input/30 p-1 text-sm transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 has-aria-invalid:border-destructive/50 md:min-h-8",
        className,
      )}
      {...props}
    />
  );
}

/** One picked item in `ComboboxChips`, with an X that unpicks it, named `remove`. */
function ComboboxChip({
  remove,
  className,
  children,
  ...props
}: ComboboxPrimitive.Chip.Props & { remove: string }) {
  return (
    <ComboboxPrimitive.Chip
      data-slot="combobox-chip"
      className={cn(
        "flex h-6 items-center gap-0.5 rounded-md border bg-secondary ps-2 pe-0.5 text-xs whitespace-nowrap outline-none data-highlighted:bg-accent",
        className,
      )}
      {...props}
    >
      {children}
      <ComboboxPrimitive.ChipRemove
        aria-label={remove}
        className="flex size-5 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground"
      >
        <X aria-hidden strokeWidth={1.5} className="size-3" />
      </ComboboxPrimitive.ChipRemove>
    </ComboboxPrimitive.Chip>
  );
}

/** What's typed in `ComboboxChips` to narrow the list. 16px on a phone, where iOS zooms otherwise. */
function ComboboxChipsInput({ className, ...props }: ComboboxPrimitive.Input.Props) {
  return (
    <ComboboxPrimitive.Input
      data-slot="combobox-chip-input"
      className={cn(
        "h-8 min-w-16 flex-1 bg-transparent ps-1.5 text-base outline-none placeholder:text-muted-foreground md:h-6 md:text-sm",
        className,
      )}
      {...props}
    />
  );
}

const ComboboxValue = ComboboxPrimitive.Value;

export {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxItem,
  ComboboxTrigger,
  ComboboxValue,
};
