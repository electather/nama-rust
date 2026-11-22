import { useEffect, useId, useRef } from "react";

import { CreateLibrary } from "@/features/admin-libraries/components/create-library";

/** The form that creates a Library, in place of the list. */
export function NewLibrary() {
  const titleId = useId();
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  return (
    <section aria-labelledby={titleId} className="flex flex-col p-4 md:px-6 md:pt-2 md:pb-6">
      {/* The crumbs over the pane name it already, so here its title is only said. */}
      <h3 ref={title} id={titleId} tabIndex={-1} className="sr-only">
        New Library
      </h3>
      <CreateLibrary />
    </section>
  );
}
