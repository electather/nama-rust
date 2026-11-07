import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormField } from "@/components/form-field";
import { Role } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { type FormErrors, formText, onSubmitted } from "@/lib/forms";
import { createUserErrors, usersKey } from "@/lib/users";

/** The form an Admin creates a User with: their name, starting password and Role. */
export function CreateUser() {
  const queryClient = useQueryClient();
  const create = useMutation(UserService.method.createUser, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: usersKey }),
  });
  const errors = create.error ? createUserErrors(create.error) : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        create.mutate(
          {
            name: formText(typed, "name"),
            password: formText(typed, "password"),
            role: formText(typed, "role") === "admin" ? Role.ADMIN : Role.MEMBER,
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

function NewUserFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField
        label="Username"
        name="name"
        autoComplete="off"
        maxLength={64}
        error={errors.name}
      />
      <FormField
        label="Starting password"
        name="password"
        type="password"
        autoComplete="new-password"
        hint="Pass it on to them. They can change it in Settings."
        error={errors.password}
      />
      <fieldset className="flex flex-col gap-2">
        <legend className="mb-2 text-sm font-medium">Role</legend>
        <RoleChoice value="member" label="Member: watches and makes Requests" defaultChecked />
        <RoleChoice value="admin" label="Admin: also manages the Server and its Users" />
      </fieldset>
    </>
  );
}

function RoleChoice({
  value,
  label,
  defaultChecked = false,
}: {
  value: string;
  label: string;
  defaultChecked?: boolean;
}) {
  return (
    <label className="flex items-center gap-3 text-sm">
      <input
        type="radio"
        name="role"
        value={value}
        defaultChecked={defaultChecked}
        className="size-4 accent-foreground"
      />
      {label}
    </label>
  );
}
