import { useEffect, useRef, useState } from "react";

/**
 * `initial` as state the field changes. The hidden inputs are controlled, so a reset of the form
 * holding the returned fieldset ref (CreateLibrary's, after a create) leaves them be: the order goes back here.
 */
export function useOrderResetWithForm(initial: string[]) {
  const [order, setOrder] = useState(initial);
  const fieldset = useRef<HTMLFieldSetElement>(null);
  useEffect(() => {
    const form = fieldset.current?.form;
    const reset = () => {
      setOrder(initial);
    };
    form?.addEventListener("reset", reset);
    return () => form?.removeEventListener("reset", reset);
  }, [initial]);
  return [order, setOrder, fieldset] as const;
}
