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
  quantity: number;
  reserve_zat: number;
  open_height: number | null;
  close_height: number;
  status: AuctionStatus;
  escrow_ua: string | null;
  trustees: TrusteeView[];
}

/** GET /v1/auctions/{code}/result (sealforge-api PublishedResult). */
export interface ResultBid {
  txid: string;
  output_index: number;
  block_height: number;
  amount_zat: number;
  commitment: string;
  leaf_hash: string;
  is_winner: boolean;
  price_paid_zat: number;
  refund_due_zat: number;
}

export interface PublishedResult {
  auction_code: string;
  network: Network;
  rules: { mode: AuctionMode; quantity: number; reserve_zat: number };
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
}
