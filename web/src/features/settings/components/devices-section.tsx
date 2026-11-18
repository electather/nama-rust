import { DeviceList } from "@/shared/components/device-list/device-list";

/** The Devices heading over the list of where the User is signed in. */
export function DevicesSection() {
  return (
    <section aria-labelledby="devices" className="mt-12 max-w-2xl">
      <h2 id="devices" className="font-medium">
        Devices
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        Where you’re signed in. Revoking a Device signs it out.
      </p>
      <div className="mt-4">
        <DeviceList />
      </div>
    </section>
  );
}
