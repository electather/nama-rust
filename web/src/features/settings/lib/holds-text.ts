/** A field of another kind, such as a checkbox, loses nothing to Escape. */
const textFieldSelector =
  "textarea, input:is(:not([type]), [type=text], [type=search], [type=email], [type=url], [type=tel], [type=password], [type=number])";

function textField(target: EventTarget) {
  const isField = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
  return isField && target.matches(textFieldSelector) ? target : undefined;
}

/**
 * Whether closing from the text field `target` would throw typed text away. Its whole form counts:
 * Escape at the autofill list of an empty Starting password would lose the Username typed above it.
 */
export function holdsText(target: EventTarget) {
  const field = textField(target);
  if (field === undefined) {
    return false;
  }
  const fields = field.form === null ? [field] : [...field.form.elements];
  return fields.some((each) => (textField(each)?.value ?? "") !== "");
}
