import { Tooltip as TooltipPrimitive } from "@base-ui/react/tooltip";
import { cn } from "cn";

// shadcn's base-nova tooltip, trimmed to the parts we use. DESIGN.md, "Menus": Popover with a
// hairline and `shadow-md`, opening on an exponential ease-out. Base UI keeps it open while the
// pointer moves onto it and closes it on Escape (WCAG 1.4.13).

function Tooltip(props: TooltipPrimitive.Root.Props) {
  return <TooltipPrimitive.Root data-slot="tooltip" {...props} />;
}

/** What opens the tooltip on hover or focus: a button, focusable, ringed when focused. */
function TooltipTrigger({ className, ...props }: TooltipPrimitive.Trigger.Props) {
  return (
    <TooltipPrimitive.Trigger
      data-slot="tooltip-trigger"
      className={cn(
        "cursor-default rounded-full outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
        className,
      )}
      {...props}
    />
  );
}

function TooltipContent({
  sideOffset = 4,
  className,
  ...props
}: TooltipPrimitive.Popup.Props & Pick<TooltipPrimitive.Positioner.Props, "sideOffset">) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Positioner className="isolate z-50" sideOffset={sideOffset}>
        {/* Base UI gives the popup no role. */}
        <TooltipPrimitive.Popup
          role="tooltip"
          data-slot="tooltip-content"
          className={cn(
            "z-50 max-w-xs origin-(--transform-origin) rounded-lg border bg-popover px-3 py-1.5 text-sm text-pretty text-popover-foreground shadow-md duration-150 ease-out-expo motion-reduce:animate-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0",
            className,
          )}
          {...props}
        />
      </TooltipPrimitive.Positioner>
    </TooltipPrimitive.Portal>
  );
}

export { Tooltip, TooltipContent, TooltipTrigger };
