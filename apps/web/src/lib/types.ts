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
  | "failed";

/** GET /v1/console/auctions/{code}/batches */
export interface ConsoleBatch {
  id: string;
  kind: "refund" | "payout";
  status: BatchStatus;
  output_count: number | null;
  total_out_zat: number | null;
  committed: number;
  signed: number;
  txid: string | null;
  confirmed_height: number | null;
}

/** GET /v1/auctions/{code}/batches (trustee view). */
export interface TrusteeBatch {
  id: string;
  auction_code: string;
  kind: "refund" | "payout";
  status: BatchStatus;
  sighash: string;
  spends: { action_index: number; alpha: string }[];
  total_out_zat: number;
  output_count: number;
  signer_slots: number[];
  signer_commitments: Record<string, string[]>;
  i_committed: boolean;
  i_shared: boolean;
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
