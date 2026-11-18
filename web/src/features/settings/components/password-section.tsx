import { ChangePassword } from "@/features/settings/components/change-password";

/** The Password heading over the form that changes it. */
export function PasswordSection() {
  return (
    <section aria-labelledby="password" className="mt-10 max-w-sm">
      <h2 id="password" className="font-medium">
        Password
      </h2>
      <ChangePassword />
    </section>
  );
}
