import { Switch as SwitchPrimitive } from "@base-ui/react/switch";

// shadcn's switch: the Rim at 80% off, Near-white on, with a knob that slides across.

function Switch(props: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className="inline-flex h-4.5 w-8 shrink-0 items-center rounded-full border border-transparent bg-input/80 transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 data-checked:bg-primary"
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="block size-4 rounded-full bg-foreground transition-transform duration-150 ease-out-expo motion-reduce:transition-none data-checked:translate-x-3.5 data-checked:bg-primary-foreground rtl:data-checked:-translate-x-3.5"
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
