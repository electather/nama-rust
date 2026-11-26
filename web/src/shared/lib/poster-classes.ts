/** The `sizes` of a `Poster` in `posterGrid`: a column at each of its breakpoints. */
export const posterGridSizes =
  "(min-width: 1024px) 183px, (min-width: 768px) 22vw, (min-width: 640px) 30vw, 45vw";

/**
 * The classes of a grid of posters: 2 columns, 3 from `sm`, 4 from `md` and 5 from `lg`, as
 * `posterColumns` counts them.
 */
export const posterGrid =
  "mt-8 grid grid-cols-2 gap-x-4 gap-y-8 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5";
