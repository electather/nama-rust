import { Menu as MenuPrimitive } from "@base-ui/react/menu";
import { cn } from "cn";
import { Check } from "lucide-react";

// shadcn's base-nova dropdown menu, trimmed to the parts we use. DESIGN.md: menus float, so they
// are Raised black with a hairline and a shadow, and open on an exponential ease-out; a
// highlighted item turns Muted. Items are 44px tall, as large as a fingertip.

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
            "z-50 max-h-(--available-height) min-w-48 origin-(--transform-origin) overflow-y-auto rounded-xl border bg-popover p-1 text-popover-foreground shadow-lg duration-150 ease-out-expo outline-none data-closed:animate-out data-closed:fade-out-0 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 motion-reduce:animate-none",
            className,
          )}
          {...props}
        />
      </MenuPrimitive.Positioner>
    </MenuPrimitive.Portal>
  );
}

const itemClassName =
  "flex min-h-11 cursor-default items-center gap-2 rounded-lg px-3 text-sm outline-hidden select-none data-highlighted:bg-muted data-disabled:pointer-events-none data-disabled:text-muted-foreground";

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
      className={cn(itemClassName, "ps-2", className)}
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

export {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
};
