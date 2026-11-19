/**
 * A group of radio buttons named `name`, as `[value, label]` pairs. The one valued `chosen` is
 * chosen, or else the first. With `onChoose` the caller keeps `chosen` up to date; without it the
 * group only starts there, for a form to read.
 */
export function Choices({
  legend,
  name,
  choices,
  chosen = choices[0]?.[0],
  onChoose,
}: {
  legend: string;
  name: string;
  choices: [value: string, label: string][];
  chosen?: string | undefined;
  onChoose?: (value: string) => void;
}) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 text-sm font-medium">{legend}</legend>
      {choices.map(([value, label]) => (
        <label key={value} className="flex items-center gap-3 text-sm">
          <input
            type="radio"
            name={name}
            value={value}
            {...(onChoose ? { checked: value === chosen } : { defaultChecked: value === chosen })}
            onChange={() => onChoose?.(value)}
            className="size-4 accent-foreground"
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}
