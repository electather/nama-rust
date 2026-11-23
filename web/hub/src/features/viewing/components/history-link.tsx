import { Link } from "@tanstack/react-router";

/** Where the Account page leads to its viewing history. */
export function HistoryLink() {
  return (
    <section aria-labelledby="your-history" className="flex flex-col gap-2">
      <h2 id="your-history" className="text-lg/6 font-medium">
        Viewing history
      </h2>
      <p className="text-sm text-muted-foreground">
        What your Linked Users play.{" "}
        <Link to="/history" className="text-primary underline underline-offset-3">
          See, delete or export it
        </Link>
      </p>
    </section>
  );
}
