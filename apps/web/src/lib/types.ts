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
