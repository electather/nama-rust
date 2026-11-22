import { useCreateUserFromAccount } from "@/features/admin-users/data/use-create-user-from-account";
import { createFromAccountErrors } from "@/features/admin-users/lib/user-refusals";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";

/**
 * The form that creates a User from a Hub Account, with no password: the Account accepts them on
 * the Hub, then they sign in with it. Only for a Server Linked to the Hub.
 */
export function CreateUserFromAccount() {
  const create = useCreateUserFromAccount();
  const errors = create.error ? createFromAccountErrors(create.error) : {};

  return (
    <form
      aria-label="Create a User from an Account"
      className="mt-4 flex max-w-sm flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        create.mutate(
          { name: formText(typed, "name"), email: formText(typed, "email") },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <FormField
        label="Username"
        name="name"
        autoComplete="off"
        maxLength={64}
        error={errors.name}
      />
      <FormField
        label="Their Account’s email"
        name="email"
        type="email"
        autoComplete="off"
        hint="They accept on the Hub, then sign in with their Account. Whether an Account has this email isn’t shown."
      />
      <FormEnd
        action={create.isPending ? "Creating User…" : "Create from Account"}
        pending={create.isPending}
        done={
          create.isSuccess
            ? `${create.variables.name} can sign in once their Account accepts on the Hub.`
            : undefined
        }
        refusal={errors.form}
      />
    </form>
  );
}
