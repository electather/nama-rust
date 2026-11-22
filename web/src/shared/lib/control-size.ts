/** How wide a setting row's control is: a field's, a number's, or as wide as it draws itself. */
export type ControlSize = "field" | "number" | "own";

/** The size of a row's input of `type`: a number's for numbers, else a field's. */
export function inputSize(type: string | undefined): ControlSize {
  return type === "number" ? "number" : "field";
}
