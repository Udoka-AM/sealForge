//! Trustee signing of escrow batches (task 3.9).
//!
//! Round 1: each trustee posts one FROST commitment per escrow spend. The
//! first two trustees to commit become the signer set. Round 2: those two post
//! one signature share per spend. The chain worker then aggregates, applies,
//! finalizes and broadcasts (see `scanner`). Nonces never leave trustees.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, State},
    Json,
};
use sealforge_api::types::{BatchView, ConsoleBatch, PostCommitments, PostShares, SpendAuthView};
use sealforge_escrow::frost::{round1::SigningCommitments, round2::SignatureShare};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{auth::Signed, error::ApiError, AppState};

#[derive(Debug, FromRow)]
struct BatchRow {
    id: Uuid,
    auction_id: Uuid,
    code: String,
    kind: String,
    status: String,
    sighash: Option<Vec<u8>>,
    spend_auth: serde_json::Value,
    total_out_zat: Option<i64>,
    output_count: Option<i32>,
    signer_slots: Option<Vec<i16>>,
}

/// Shared SELECT for `BatchRow`, spliced in at compile time so queries stay static.
macro_rules! batch_select {
    () => {
        "SELECT b.id, b.auction_id, a.code, b.kind::text AS kind, b.status::text AS status, b.sighash, \
         b.spend_auth, b.total_out_zat, b.output_count, b.signer_slots \
         FROM tx_batches b JOIN auctions a ON a.id = b.auction_id"
    };
}

/// The trustee (id, slot) behind a request for this auction.
async fn trustee_of(
    state: &AppState,
    pubkey: &[u8; 32],
    auction_id: Uuid,
) -> Result<(Uuid, i16), ApiError> {
    sqlx::query_as("SELECT id, slot FROM trustees WHERE auction_id = $1 AND pubkey = $2")
        .bind(auction_id)
        .bind(&pubkey[..])
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)
}

fn spends(row: &BatchRow) -> Result<Vec<SpendAuthView>, ApiError> {
    serde_json::from_value(row.spend_auth.clone()).map_err(|e| ApiError::Internal(e.into()))
}

async fn view(state: &AppState, row: &BatchRow, trustee_id: Uuid) -> Result<BatchView, ApiError> {
    let rows: Vec<(i16, Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT t.slot, s.commitments_json, s.shares_json FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
         WHERE s.batch_id = $1",
    )
    .bind(row.id)
    .fetch_all(&state.db)
    .await?;
    let my_slot: i16 = sqlx::query_scalar("SELECT slot FROM trustees WHERE id = $1")
        .bind(trustee_id)
        .fetch_one(&state.db)
        .await?;
    let signers: Vec<i16> = row.signer_slots.clone().unwrap_or_default();
    let mut signer_commitments = BTreeMap::new();
    for (slot, commitments, _) in &rows {
        if signers.contains(slot) {
            if let Some(c) = commitments {
                signer_commitments.insert(
                    *slot as u16,
                    serde_json::from_value(c.clone()).unwrap_or_default(),
                );
            }
        }
    }
    let mine = rows.iter().find(|(slot, _, _)| *slot == my_slot);
    Ok(BatchView {
        id: row.id.to_string(),
        auction_code: row.code.clone(),
        kind: row.kind.clone(),
        status: row.status.clone(),
        sighash: hex::encode(row.sighash.clone().unwrap_or_default()),
        spends: spends(row)?,
        total_out_zat: row.total_out_zat.unwrap_or_default() as u64,
        output_count: row.output_count.unwrap_or_default() as u32,
        signer_slots: signers.iter().map(|s| *s as u16).collect(),
        signer_commitments,
        i_committed: mine.is_some_and(|(_, c, _)| c.is_some()),
        i_shared: mine.is_some_and(|(_, _, s)| s.is_some()),
    })
}

/// `GET /v1/auctions/{code}/batches`: batches this trustee may need to sign.
pub async fn list(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<Vec<BatchView>>, ApiError> {
    let auction_id: Uuid = sqlx::query_scalar("SELECT id FROM auctions WHERE code = $1")
        .bind(&code)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let (trustee_id, _) = trustee_of(&state, &req.pubkey, auction_id).await?;
    let rows: Vec<BatchRow> = sqlx::query_as(concat!(
        batch_select!(),
        " WHERE b.auction_id = $1 AND b.status IN ('awaiting_signatures', 'signed') ORDER BY b.created_at"
    ))
    .bind(auction_id)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::new();
    for row in &rows {
        out.push(view(&state, row, trustee_id).await?);
    }
    Ok(Json(out))
}

async fn load(state: &AppState, id: &str) -> Result<BatchRow, ApiError> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::NotFound)?;
    sqlx::query_as(concat!(batch_select!(), " WHERE b.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)
}

fn decode_each<T>(
    values: &[String],
    count: usize,
    parse: impl Fn(&[u8]) -> Option<T>,
) -> Result<(), ApiError> {
    if values.len() != count {
        return Err(ApiError::BadRequest(format!(
            "expected {count} values, one per spend"
        )));
    }
    for v in values {
        let bytes =
            hex::decode(v).map_err(|_| ApiError::BadRequest("values must be hex".into()))?;
        parse(&bytes).ok_or_else(|| ApiError::BadRequest("malformed FROST value".into()))?;
    }
    Ok(())
}

/// `POST /v1/batches/{id}/commitments` (round 1).
pub async fn commit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req: Signed,
) -> Result<Json<BatchView>, ApiError> {
    let row = load(&state, &id).await?;
    let (trustee_id, _) = trustee_of(&state, &req.pubkey, row.auction_id).await?;
    if row.status != "awaiting_signatures" {
        return Err(ApiError::conflict(
            "not_signing",
            "this batch is not collecting signatures",
        ));
    }
    let input: PostCommitments = req.json()?;
    decode_each(&input.commitments, spends(&row)?.len(), |b| {
        SigningCommitments::deserialize(b).ok()
    })?;

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO signature_shares (batch_id, trustee_id, commitments_json) VALUES ($1, $2, $3) \
         ON CONFLICT (batch_id, trustee_id) DO NOTHING",
    )
    .bind(row.id)
    .bind(trustee_id)
    .bind(serde_json::json!(input.commitments))
    .execute(&mut *tx)
    .await?;
    // The first two trustees to commit become the signer set.
    sqlx::query(
        "UPDATE tx_batches SET signer_slots = (
            SELECT array_agg(slot ORDER BY submitted_at, slot) FROM (
              SELECT t.slot, s.submitted_at FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id
              WHERE s.batch_id = $1 AND s.commitments_json IS NOT NULL ORDER BY s.submitted_at, t.slot LIMIT 2) x)
         WHERE id = $1 AND signer_slots IS NULL AND (
            SELECT count(*) FROM signature_shares WHERE batch_id = $1 AND commitments_json IS NOT NULL) >= 2",
    )
    .bind(row.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let row = load(&state, &id).await?;
    Ok(Json(view(&state, &row, trustee_id).await?))
}

/// `POST /v1/batches/{id}/shares` (round 2, signers only).
pub async fn share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req: Signed,
) -> Result<Json<BatchView>, ApiError> {
    let row = load(&state, &id).await?;
    let (trustee_id, slot) = trustee_of(&state, &req.pubkey, row.auction_id).await?;
    if row.status != "awaiting_signatures" {
        return Err(ApiError::conflict(
            "not_signing",
            "this batch is not collecting signatures",
        ));
    }
    if !row.signer_slots.clone().unwrap_or_default().contains(&slot) {
        return Err(ApiError::conflict(
            "not_a_signer",
            "two other trustees are signing this batch",
        ));
    }
    let input: PostShares = req.json()?;
    decode_each(&input.shares, spends(&row)?.len(), |b| {
        SignatureShare::deserialize(b).ok()
    })?;

    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE signature_shares SET shares_json = $3 WHERE batch_id = $1 AND trustee_id = $2 AND shares_json IS NULL")
        .bind(row.id)
        .bind(trustee_id)
        .bind(serde_json::json!(input.shares))
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE tx_batches SET status = 'signed' WHERE id = $1 AND status = 'awaiting_signatures' AND (
            SELECT count(*) FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id
            WHERE s.batch_id = $1 AND s.shares_json IS NOT NULL AND t.slot = ANY(tx_batches.signer_slots)) = 2",
    )
    .bind(row.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let row = load(&state, &id).await?;
    Ok(Json(view(&state, &row, trustee_id).await?))
}

#[derive(Debug, FromRow)]
struct ConsoleBatchRow {
    id: Uuid,
    kind: String,
    status: String,
    output_count: Option<i32>,
    total_out_zat: Option<i64>,
    committed: i64,
    signed: i64,
    txid: Option<Vec<u8>>,
    confirmed_height: Option<i32>,
}

/// `GET /v1/console/auctions/{code}/batches`: escrow transactions for the
/// auction's organiser, oldest first. Status and totals only.
pub async fn list_for_organiser(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<Vec<ConsoleBatch>>, ApiError> {
    let auction_id: Uuid = sqlx::query_scalar(
        "SELECT a.id FROM auctions a JOIN organisers o ON o.id = a.organiser_id WHERE a.code = $1 AND o.pubkey = $2",
    )
    .bind(&code)
    .bind(&req.pubkey[..])
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;
    let rows: Vec<ConsoleBatchRow> = sqlx::query_as(
        "SELECT b.id, b.kind::text AS kind, b.status::text AS status, b.output_count, b.total_out_zat, \
         (SELECT count(*) FROM signature_shares s WHERE s.batch_id = b.id AND s.commitments_json IS NOT NULL) AS committed, \
         (SELECT count(*) FROM signature_shares s WHERE s.batch_id = b.id AND s.shares_json IS NOT NULL) AS signed, \
         b.txid, b.confirmed_height FROM tx_batches b WHERE b.auction_id = $1 ORDER BY b.created_at",
    )
    .bind(auction_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| ConsoleBatch {
                id: r.id.to_string(),
                kind: r.kind,
                status: r.status,
                output_count: r.output_count.map(|c| c as u32),
                total_out_zat: r.total_out_zat.map(|t| t as u64),
                committed: r.committed as u16,
                signed: r.signed as u16,
                // Stored in internal byte order; wallets and explorers show it reversed.
                txid: r.txid.map(|mut t| {
                    t.reverse();
                    hex::encode(t)
                }),
                confirmed_height: r.confirmed_height.map(|h| h as u32),
            })
            .collect(),
    ))
}
