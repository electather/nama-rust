import { FeaturedFrame } from "@/features/home/components/featured-frame";
import { openGrow } from "@/features/home/lib/accordion";

/** Where Featured's panels will be while they load: the first open, the rest closed. */
export function FeaturedSkeleton() {
  return (
    <div aria-hidden>
      <FeaturedFrame label="Featured">
        {[0, 1, 2, 3, 4].map((index) => (
          <li
            key={index}
            className="min-w-0 grow-(--grow) basis-0 animate-pulse rounded-3xl bg-muted motion-reduce:animate-none max-[720px]:min-h-21 max-[720px]:first:min-h-130"
            style={{ "--grow": index === 0 ? openGrow(5) : 1 }}
          />
        ))}
      </FeaturedFrame>
    </div>
  );
}
