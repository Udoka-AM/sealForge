import type { ReactNode } from "react";

/**
 * The one way a home-page section opens, on the 12-column grid: a hairline rule, then the index
 * and label (3 columns), the title (6) and a lead or action (3), bottom-aligned.
 */
export function SectionHead({
  index,
  label,
  title,
  titleId,
  lead,
}: {
  index: string;
  label: string;
  title: ReactNode;
  titleId: string;
  lead?: ReactNode;
}) {
  return (
    <div className="grid gap-x-8 gap-y-4 border-t border-border pt-5 md:grid-cols-12">
      <p className="eyebrow flex gap-3 md:col-span-3">
        <span className="text-primary">{index}</span>
        <span>{label}</span>
      </p>
      <h2 id={titleId} className="display-2 md:col-span-6">
        {title}
      </h2>
      {lead ? (
        <div className="text-[15px] leading-6 text-muted md:col-span-3 md:self-end">{lead}</div>
      ) : null}
    </div>
  );
}
