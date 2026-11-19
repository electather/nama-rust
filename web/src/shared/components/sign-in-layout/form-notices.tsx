import { Notice } from "@/shared/components/sign-in-layout/notice";
import type { FormErrors } from "@/shared/lib/forms";

/** The Server's refusal of a whole form, as notices: `wait` as waiting, `form` as an error. */
export function FormNotices({ errors }: { errors: FormErrors }) {
  return (
    <>
      {errors.wait !== undefined && <Notice kind="wait">{errors.wait}</Notice>}
      {errors.form !== undefined && <Notice kind="error">{errors.form}</Notice>}
    </>
  );
}
