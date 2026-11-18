import { Menu as MenuPrimitive } from "@base-ui/react/menu";
import { cn } from "cn";
import { Check } from "lucide-react";

// shadcn's base-nova dropdown menu, trimmed to the parts we use. DESIGN.md, "Menus": `popover`
// with a hairline and `shadow-pop`, opening on an exponential ease-out; a highlighted item turns
// `muted`, and the chosen one is semibold.

function DropdownMenu(props: MenuPrimitive.Root.Props) {
  return <MenuPrimitive.Root data-slot="dropdown-menu" {...props} />;
}

function DropdownMenuTrigger(props: MenuPrimitive.Trigger.Props) {
  return <MenuPrimitive.Trigger data-slot="dropdown-menu-trigger" {...props} />;
}

function DropdownMenuContent({
  align = "end",
  sideOffset = 4,
  container,
  className,
  ...props
}: MenuPrimitive.Popup.Props &
  Pick<MenuPrimitive.Positioner.Props, "align" | "sideOffset"> &
  Pick<MenuPrimitive.Portal.Props, "container">) {
  return (
    <MenuPrimitive.Portal container={container}>
      <MenuPrimitive.Positioner
        className="isolate z-50 outline-none"
        align={align}
        sideOffset={sideOffset}
      >
        <MenuPrimitive.Popup
          data-slot="dropdown-menu-content"
          className={cn(
            "z-50 grid max-h-(--available-height) min-w-60 origin-(--transform-origin) gap-0.5 overflow-y-auto rounded-xl border bg-popover p-1 text-popover-foreground shadow-pop duration-150 ease-out-expo outline-none motion-reduce:animate-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0",
            className,
          )}
          {...props}
        />
      </MenuPrimitive.Positioner>
    </MenuPrimitive.Portal>
  );
}

const itemClassName =
  "flex min-h-10 cursor-default items-center gap-2.5 rounded-sm px-2.5 text-sm text-foreground-soft outline-hidden select-none data-checked:font-semibold data-checked:text-foreground data-disabled:pointer-events-none data-disabled:text-muted-foreground data-highlighted:bg-muted data-highlighted:text-foreground [&_svg]:text-muted-foreground data-highlighted:[&_svg]:text-foreground-soft";

function DropdownMenuItem({ className, ...props }: MenuPrimitive.Item.Props) {
  return (
    <MenuPrimitive.Item
      data-slot="dropdown-menu-item"
      className={cn(itemClassName, className)}
      {...props}
    />
  );
}

/** Holds the radio items of which one is chosen at a time. */
function DropdownMenuRadioGroup(props: MenuPrimitive.RadioGroup.Props) {
  return <MenuPrimitive.RadioGroup data-slot="dropdown-menu-radio-group" {...props} />;
}

/** An item that stays in the menu once chosen, with a check where the chosen one is. */
function DropdownMenuRadioItem({ className, children, ...props }: MenuPrimitive.RadioItem.Props) {
  return (
    <MenuPrimitive.RadioItem
      data-slot="dropdown-menu-radio-item"
      className={cn(itemClassName, className)}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <MenuPrimitive.RadioItemIndicator>
          <Check aria-hidden className="size-4" />
        </MenuPrimitive.RadioItemIndicator>
      </span>
      {children}
    </MenuPrimitive.RadioItem>
  );
}

/** An item that turns its choice on or off and leaves the menu open, with a check when on. */
function DropdownMenuCheckboxItem({
  className,
  children,
  ...props
}: MenuPrimitive.CheckboxItem.Props) {
  return (
    <MenuPrimitive.CheckboxItem
      data-slot="dropdown-menu-checkbox-item"
      className={cn(itemClassName, className)}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <MenuPrimitive.CheckboxItemIndicator>
          <Check aria-hidden className="size-4" />
        </MenuPrimitive.CheckboxItemIndicator>
      </span>
      {children}
    </MenuPrimitive.CheckboxItem>
  );
}

export {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
};
