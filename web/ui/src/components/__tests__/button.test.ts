import { buttonVariants } from "nama-ui/components/button";
import { describe, expect, test } from "vite-plus/test";

// A `<Link className={buttonVariants(...)}>` gets the string as it is, with no `cn` after it.
describe("a link styled as a button", () => {
  test("an outline one keeps its Rim", () => {
    const classes = buttonVariants({ variant: "outline" }).split(" ");

    expect(classes).toContain("border-input");
    expect(classes).not.toContain("border-transparent");
  });

  test.each([
    ["link", "rounded-sm"],
    ["close", "rounded-sm"],
    ["icon", "rounded-lg"],
    ["icon-sm", "rounded-lg"],
  ] as const)("size %s keeps its %s corners", (size, corners) => {
    const classes = buttonVariants({ size }).split(" ");

    expect(classes).toContain(corners);
    expect(classes).not.toContain("rounded-full");
  });
});
