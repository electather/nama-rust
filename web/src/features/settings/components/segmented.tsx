/**
 * A few choices in one pill, of which `value` is chosen, raised on the Rim. Native radios named
 * `name`, so the arrow keys move between them.
 */
export function Segmented<Value extends string>({
  name,
  options,
  value,
  onChange,
  ...group
}: {
  name: string;
  options: [value: Value, label: string][];
  value: Value;
  onChange: (value: Value) => void;
  "aria-labelledby": string;
  "aria-describedby"?: string;
}) {
  return (
    <div role="radiogroup" {...group} className="flex h-11 rounded-lg bg-muted p-0.75 md:h-7">
      {options.map(([each, label]) => (
        <label
          key={each}
          className="flex flex-auto cursor-pointer items-center justify-center rounded-md border border-transparent px-2 text-sm whitespace-nowrap transition-colors has-checked:border-input has-checked:bg-input/30 has-checked:shadow-xs has-focus-visible:border-ring has-focus-visible:ring-3 has-focus-visible:ring-ring/50 motion-reduce:transition-none"
        >
          <input
            type="radio"
            name={name}
            value={each}
            checked={each === value}
            onChange={() => {
              onChange(each);
            }}
            className="sr-only"
          />
          {label}
        </label>
      ))}
    </div>
  );
}
