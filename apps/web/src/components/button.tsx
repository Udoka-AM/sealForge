import Link from "next/link";
import type { ComponentProps } from "react";

const base =
  "inline-flex items-center justify-center gap-2 rounded-[var(--radius-control)] px-5 font-medium transition-[background-color,border-color,color] duration-150 disabled:cursor-not-allowed disabled:opacity-40 touch-manipulation";

const variants = {
  primary: "bg-primary text-primary-ink hover:brightness-110",
  secondary: "border border-border bg-surface text-text hover:border-muted",
  ghost: "text-muted hover:text-text",
} as const;

const sizes = { md: "h-11 text-[15px]", lg: "h-[52px] text-base" } as const;

type Style = { variant?: keyof typeof variants; size?: keyof typeof sizes };

export function buttonClass({ variant = "primary", size = "md" }: Style = {}) {
  return `${base} ${variants[variant]} ${sizes[size]}`;
}

export function Button({
  variant,
  size,
  className = "",
  ...props
}: ComponentProps<"button"> & Style) {
  return <button className={`${buttonClass({ variant, size })} ${className}`} {...props} />;
}

export function ButtonLink({
  variant,
  size,
  className = "",
  ...props
}: ComponentProps<typeof Link> & Style) {
  return <Link className={`${buttonClass({ variant, size })} ${className}`} {...props} />;
}
