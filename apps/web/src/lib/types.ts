/** Mirrors sealforge-api's /v1 types (crates/sealforge-api/src/types.rs). */

export type Network = "regtest" | "testnet" | "mainnet";
export type AuctionMode = "uniform_price" | "first_price";
export type AuctionStatus =
  | "draft"
  | "dkg"
  | "ready"
  | "open"
  | "closed"
  | "published"
  | "refunding"
  | "settled"
  | "cancelled";

export interface TrusteeView {
  slot: number;
  label: string;
  dkg_status: "invited" | "round1" | "round2" | "complete";
  /** The trustee's Ed25519 key for this auction (hex). */
  pubkey?: string | null;
  /** The trustee's signature over the escrow address (hex). */
  escrow_attestation?: string | null;
  /** The trustee's long-term key vouching for `pubkey` in this auction. */
  identity?: TrusteeIdentityCert | null;
}

/** A trustee's long-term key (hex), where it publishes it, and its certificate. */
export interface TrusteeIdentityCert {
  pubkey: string;
  published_at: string;
  signature: string;
}

export interface AuctionView {
  code: string;
  title: string;
  description: string;
  lot_image_url: string | null;
  network: Network;
  mode: AuctionMode;
  bid_mode: "deposit" | "commit_reveal";
  /** Commit-reveal only: last block for reveals. */
  reveal_close_height: number | null;
  quantity: number;
  reserve_zat: number;
  open_height: number | null;
  close_height: number;
  status: AuctionStatus;
  escrow_ua: string | null;
  trustees: TrusteeView[];
  /** Hidden (null) below 5 bids so early activity stays private. */
  bid_count: number | null;
  /** Only in the organiser's own view (GET /v1/console/...). */
  owner?: { payout_ua: string; fee_buffer_zat: number };
  /** sealForge's fee on proceeds, in basis points (absent: none). */
  fee_bps?: number;
  /** Where the fee is paid. */
  fee_ua?: string | null;
}

export interface TrusteeInvite {
  slot: number;
  label: string;
  invite_code: string;
}

/** POST /v1/auctions */
export interface CreateAuction {
  title: string;
  description: string;
  lot_image_url: string | null;
  network: Network;
  mode: AuctionMode;
  bid_mode: "deposit" | "commit_reveal";
  reveal_close_height: number | null;
  quantity: number;
  reserve_zat: number;
  close_height: number;
  payout_ua: string;
  trustees: { slot: number; label: string }[];
}

export interface CreatedAuction {
  auction: AuctionView;
  trustee_invites: TrusteeInvite[];
}

/** GET /v1/chain */
export interface ChainView {
  tip: number | null;
}

/** GET /v1/auctions/{code}/refunds (aggregates only). */
export interface RefundProgress {
  notes_total: number;
  notes_planned: number;
  notes_batched: number;
  notes_sent: number;
  batches: number;
  needs_manual_review: number;
}

export type BatchStatus =
  | "building"
  | "awaiting_signatures"
  | "signed"
  | "broadcast"
  | "confirmed"
  | "failed"
  | "expired";

/** GET /v1/console/auctions/{code}/batches */
/** `consolidate` merges escrow notes inside the escrow so a refund fits in one transaction. */
export type BatchKind = "refund" | "payout" | "consolidate";

export const BATCH_KIND_LABEL: Record<BatchKind, string> = {
  refund: "Refund",
  payout: "Payout",
  consolidate: "Note merge",
};

export interface ConsoleBatch {
  id: string;
  kind: BatchKind;
  status: BatchStatus;
  output_count: number | null;
  total_out_zat: number | null;
  committed: number;
  signed: number;
  txid: string | null;
  confirmed_height: number | null;
}

/** GET /v1/auctions/{code}/batches (trustee view). */
/** GET /v1/auctions/{code}/dkg: only the field the signing page needs. */
export interface TrusteeDkgState {
  escrow_ua: string | null;
  /** `nk ‖ rivk`, sealed to this trustee's encryption key (hex). */
  escrow_view_secret_for_me?: string | null;
}

/** What the trustee's browser checked in a batch's transaction (sealforge-verify). */
export interface VerifiedBatch {
  sighash: string;
  lines: string[];
  warnings: string[];
  payments: [string, number][];
  fee_zat: number;
  expiry_height: number;
  /** Payouts: the protocol fee this transaction pays (0 if none). */
  protocol_fee_zat?: number;
  /** Refund batches, and payouts that pay the fee: what to add to this trustee's ledger before its share leaves. */
  ledger_entry?: RefundLedgerEntry;
}

/** One refund batch this trustee signed (sealforge_verify::refunds::LedgerEntry). */
export interface RefundLedgerEntry {
  batch_txid: string;
  expiry_height: number;
  notes: { bid_txid: string; bid_output_index: number; value_zat: number }[];
  /** Payouts: the protocol fee it paid. */
  fee_zat?: number;
}

export interface TrusteeBatch {
  id: string;
  auction_code: string;
  kind: BatchKind;
  status: BatchStatus;
  sighash: string;
  spends: { action_index: number; alpha: string }[];
  total_out_zat: number;
  output_count: number;
  signer_slots: number[];
  signer_commitments: Record<string, string[]>;
  i_committed: boolean;
  i_shared: boolean;
  /** The transaction to check before signing (PCZT, hex). */
  pczt?: string | null;
  /** Payout batches: the organiser's payout address. */
  payout_ua?: string | null;
  /** Refund batches: which bid each refund note pays back. */
  refunds?: { bid_txid: string; bid_output_index: number; value_zat: number }[];
  /**
   * Each chosen signer's signed statement of the round 2 inputs it signs over
   * (digest and Ed25519 signature, hex), by slot. A signer sends its share only
   * once its co-signer's statement matches its own.
   */
  signer_inputs?: Record<string, { digest: string; signature: string }>;
}

/** GET /v1/auctions/{code}/result (sealforge-api PublishedResult). */
export interface ResultBid {
  txid: string;
  output_index: number;
  block_height: number;
  /** The bid: the payment, or in commit-reveal mode the revealed price. */
  amount_zat: number;
  /** Commit-reveal only: the deposit, and the counted reveal payment. */
  deposit_zat?: number;
  reveal_zat?: number;
  commitment: string;
  leaf_hash: string;
  is_winner: boolean;
  price_paid_zat: number;
  refund_due_zat: number;
}

export interface PublishedResult {
  auction_code: string;
  network: Network;
  rules: {
    mode: AuctionMode;
    quantity: number;
    reserve_zat: number;
    bid_mode?: "deposit" | "commit_reveal";
    /** Commit-reveal: the bidding close; scan_to_height is then the reveal close. */
    close_height?: number;
  };
  scan_from_height: number;
  scan_to_height: number;
  uivk: string;
  algorithm_version: string;
  clearing_price_zat: number | null;
  total_volume_zat: number;
  valid_bid_count: number;
  winner_count: number;
  merkle_root: string;
  bids: ResultBid[];
  /** The escrow address bidders paid. */
  /** sealForge's fee rate (basis points), its address, and the fee owed (absent: none). */
  fee_bps?: number;
  fee_ua?: string | null;
  fee_zat?: number;
  escrow_ua?: string;
  /** Each trustee's attestation over escrow_ua. */
  trustees?: {
    slot: number;
    label: string;
    pubkey: string | null;
    signature: string | null;
  }[];
}

/** sealforge-auditor EscrowCheck. */
export interface EscrowCheck {
  verdict: "verified" | "unattested" | "failed";
  escrow_ua: string | null;
  signed: number;
  total: number;
  trustees: {
    slot: number;
    label: string;
    pubkey: string | null;
    verdict: "signed" | "missing" | "invalid" | "wrong_key";
    /** Whether a long-term key vouches for this trustee's key. */
    identity?: "certified" | "none" | "invalid";
    identity_pubkey?: string | null;
    identity_published_at?: string | null;
  }[];
  /** Why the trustee list itself is unacceptable; empty when well formed. */
  problems?: string[];
}

/** sealforge-auditor AuditReport. */
export interface AuditReport {
  matches: boolean;
  fields: { field: string; published: string; recomputed: string; ok: boolean }[];
}

/** sealforge-auditor ReceiptCheck. */
export interface ReceiptCheck {
  receipt_valid: boolean;
  found: boolean;
  included: boolean;
  amount_matches: boolean;
  amount_zat: number | null;
  block_height: number | null;
  is_winner: boolean;
  price_paid_zat: number;
  refund_due_zat: number;
  refund_notes_zat: number[];
  /** Part of the refund below 0.0001 ZEC, which is not sent. */
  dust_zat: number;
}
