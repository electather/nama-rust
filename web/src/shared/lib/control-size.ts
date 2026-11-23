/** How wide a setting row's control is: a field's, a number's, or as wide as it draws itself. */
export type ControlSize = "field" | "number" | "own";

/**
 * How a setting row lays out its input of `type`: a number's size, else a field's. A field is
 * `wide`, dropping under its label on a phone, since beside it there it's too narrow to read.
 */
export function inputLayout(type: string | undefined): { size: ControlSize; wide: boolean } {
  return type === "number" ? { size: "number", wide: false } : { size: "field", wide: true };
}
