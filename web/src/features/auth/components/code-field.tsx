import { type ComponentProps, useState } from "react";

import { formatCode } from "@/features/auth/lib/format-code";
import { FormField } from "@/shared/components/form-field/form-field";

/** A field for a Setup or Sign-in Code of `length` characters, formatted as it's typed. */
export function CodeField({
  length,
  defaultValue = "",
  ...field
}: Omit<ComponentProps<typeof FormField>, "defaultValue" | "value" | "onChange"> & {
  length: number;
  defaultValue?: string | undefined;
}) {
  const [code, setCode] = useState(() => formatCode(defaultValue, length));
  return (
    <FormField
      {...field}
      variant="code"
      value={code}
      onChange={(event) => {
        setCode(formatCode(event.target.value, length));
      }}
      autoComplete="off"
      autoCapitalize="characters"
      spellCheck={false}
    />
  );
}
