import { cn } from "@/shared/lib/cn";
import CrewLogoAnimation from "@/shared/ui/crew-logo/CrewLogoAnimation";

/** Centered, low-emphasis loading state for page and panel fetches. */
export function CrewLoadingState({
  className,
  fill = false,
  label = "Loading",
}: {
  className?: string;
  fill?: boolean;
  label?: string;
}) {
  return (
    <div
      className={cn(
        "flex w-full items-center justify-center text-muted-foreground/45",
        fill ? "min-h-0 flex-1" : "min-h-[calc(100dvh-7rem)]",
        className,
      )}
      data-testid="crew-loading-state"
      role="status"
    >
      <CrewLogoAnimation
        ariaLabel={label}
        className="crew-logo--scale-pulse"
        fullScreen={false}
        showBackground={false}
        style={{ width: "2rem" }}
        textured={false}
      />
    </div>
  );
}
