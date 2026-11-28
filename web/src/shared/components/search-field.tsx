import { cn } from "cn";
import { Search, X } from "lucide-react";
import { type ComponentProps, useRef } from "react";

/**
 * A field that searches or filters (DESIGN.md, "Inputs"): the search icon and the words `value` in
 * one box, `className`'s, and while it has words a button that clears them, as Escape does before
 * anything else hears it. `onValueChange` hears each change; the rest goes to the `<input>`.
 */
export function SearchField({
  value,
  onValueChange,
  className,
  onKeyDown,
  ...input
}: Omit<ComponentProps<"input">, "value" | "onChange"> & {
  value: string;
  onValueChange: (value: string) => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  return (
    <div
      className={cn(
        "flex h-11 min-w-0 items-center gap-2 rounded-lg border border-input bg-input/30 ps-2.5 pe-1.5 text-muted-foreground transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 md:h-8 md:ps-2 md:pe-1",
        className,
      )}
    >
      <Search aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
      <input
        ref={field}
        autoComplete="off"
        spellCheck={false}
        {...input}
        value={value}
        onChange={(event) => {
          onValueChange(event.target.value);
        }}
        onKeyDown={(event) => {
          onKeyDown?.(event);
          // Settings prevents Escape in a field holding words, which stops a browser clearing it.
          if (!event.defaultPrevented && event.key === "Escape" && value !== "") {
            event.preventDefault();
            onValueChange("");
          }
        }}
        // 16px on a phone, where iOS zooms into a field with smaller text. The browser's own clear
        // button is hidden for this one's.
        className="min-w-0 flex-1 bg-transparent text-base text-foreground outline-none placeholder:text-muted-foreground md:text-sm [&::-webkit-search-cancel-button]:appearance-none"
      />
      {value !== "" && (
        <button
          type="button"
          aria-label="Clear"
          onClick={() => {
            onValueChange("");
            field.current?.focus();
          }}
          className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-full outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 md:size-5"
        >
          <X aria-hidden strokeWidth={1.5} className="size-3.5" />
        </button>
      )}
    </div>
  );
}
