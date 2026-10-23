import type { ReactNode } from "react";

/**
 * An endless strip: the list twice, sliding left by half its width. Pauses on hover; still with
 * reduced motion. The second copy is hidden from screen readers.
 */
export function Marquee({
  children,
  className = "",
  seconds = 40,
}: {
  children: ReactNode;
  className?: string;
  seconds?: number;
}) {
  return (
    <div
      className={`group relative overflow-hidden [mask-image:linear-gradient(to_right,transparent,black_8%,black_92%,transparent)] ${className}`}
    >
      <div
        className="marquee flex w-max group-hover:[animation-play-state:paused]"
        style={{ animationDuration: `${seconds}s` }}
      >
        <div className="flex shrink-0 items-center">{children}</div>
        <div className="flex shrink-0 items-center" aria-hidden="true">
          {children}
        </div>
      </div>
    </div>
  );
}
