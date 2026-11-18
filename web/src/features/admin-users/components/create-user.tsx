import { NewUserFields } from "@/features/admin-users/components/new-user-fields";
import { useCreateUser } from "@/features/admin-users/data/use-create-user";
import { createUserErrors } from "@/features/admin-users/lib/user-refusals";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form that creates a User with a starting password. */
export function CreateUser() {
  const create = useCreateUser();
  const errors = create.error ? createUserErrors(create.error) : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        create.mutate(
          {
            name: formText(typed, "name"),
            password: formText(typed, "password"),
          },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <NewUserFields errors={errors} />
      <FormEnd
        action={create.isPending ? "Creating User…" : "Create User"}
        pending={create.isPending}
        done={create.isSuccess ? `${create.variables.name} can sign in now.` : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
