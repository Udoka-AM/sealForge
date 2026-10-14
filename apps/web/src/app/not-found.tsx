import { ButtonLink } from "@/components/button";

export default function NotFound() {
  return (
    <div className="max-w-[480px] space-y-4">
      <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">Auction not found</h1>
      <p className="text-muted">
        Check the link. Auctions that haven&rsquo;t opened yet, or were cancelled, aren&rsquo;t
        public.
      </p>
      <ButtonLink href="/" variant="secondary">
        See open auctions
      </ButtonLink>
    </div>
  );
}
