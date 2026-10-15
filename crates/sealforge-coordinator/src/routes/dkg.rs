//! Trustee join and DKG relay (task 2.4).
//!
//! The coordinator relays packages but never sees secrets: round 1 packages
//! are signed by the sending trustee, and round 2 packages are also sealed to
//! the recipient's X25519 key. When all three trustees report the same public
//! key package, the coordinator derives the escrow address and viewing keys.

use std::collections::BTreeSet;

use axum::{
    extract::{Path, State},
    Json,
};
use rand::rngs::OsRng;
use sealforge_api::{
    auth::verify_dkg_package,
    types::{
        AuctionStatus, CompleteDkg, DkgPackage, DkgState, DkgStatus, DkgTrustee, JoinAsTrustee,
        PostRound1, PostRound2, TrusteeJoined,
    },
};
use sealforge_escrow::{address, dkg as frost_dkg, frost::keys::PublicKeyPackage, Threshold};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::Signed,
    codes,
    db::{self, AuctionRow},
    error::ApiError,
    lifecycle::{can_transition, Actor},
    validate, AppState,
};

#[derive(Debug, FromRow)]
struct TrusteeRow {
    id: Uuid,
    slot: i16,
    label: String,
    pubkey: Option<Vec<u8>>,
    enc_pubkey: Option<Vec<u8>>,
    dkg_status: DkgStatus,
    public_key_package: Option<Vec<u8>>,
}

#[derive(Debug, FromRow)]
struct PackageRow {
    from_slot: i16,
    to_slot: Option<i16>,
    payload: Vec<u8>,
    signature: Vec<u8>,
}

impl From<PackageRow> for DkgPackage {
    fn from(p: PackageRow) -> Self {
        Self {
            from_slot: p.from_slot as u16,
            to_slot: p.to_slot.map(|s| s as u16),
            payload: hex::encode(p.payload),
            signature: hex::encode(p.signature),
        }
    }
}

fn decode_hex<const N: usize>(value: &str, what: &str) -> Result<[u8; N], ApiError> {
    hex::decode(value)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| ApiError::BadRequest(format!("{what} must be {N} bytes of hex")))
}

fn decode_payload(value: &str) -> Result<Vec<u8>, ApiError> {
    let bytes =
        hex::decode(value).map_err(|_| ApiError::BadRequest("payload must be hex".into()))?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err(ApiError::BadRequest(
            "payload must be 1 byte to 16 KiB".into(),
        ));
    }
    Ok(bytes)
}

const THRESHOLD: Threshold = Threshold {
    min_signers: 2,
    max_signers: 3,
};

async fn trustees_of(state: &AppState, auction_id: Uuid) -> Result<Vec<TrusteeRow>, ApiError> {
    Ok(sqlx::query_as(
        "SELECT id, slot, label, pubkey, enc_pubkey, dkg_status, public_key_package \
         FROM trustees WHERE auction_id = $1 ORDER BY slot",
    )
    .bind(auction_id)
    .fetch_all(&state.db)
    .await?)
}

/// The auction and the caller's trustee row. Non-trustees get 404.
async fn as_trustee(
    state: &AppState,
    req: &Signed,
    code: &str,
) -> Result<(AuctionRow, TrusteeRow, Vec<TrusteeRow>), ApiError> {
    let auction = db::auction_by_code(&state.db, code)
        .await?
        .ok_or(ApiError::NotFound)?;
    let trustees = trustees_of(state, auction.id).await?;
    let me = trustees
        .iter()
        .find(|t| t.pubkey.as_deref() == Some(&req.pubkey[..]))
        .ok_or(ApiError::NotFound)?;
    let me = TrusteeRow {
        id: me.id,
        slot: me.slot,
        label: me.label.clone(),
        pubkey: me.pubkey.clone(),
        enc_pubkey: me.enc_pubkey.clone(),
        dkg_status: me.dkg_status,
        public_key_package: me.public_key_package.clone(),
    };
    Ok((auction, me, trustees))
}

fn require_dkg(auction: &AuctionRow) -> Result<(), ApiError> {
    if auction.status == AuctionStatus::Dkg {
        Ok(())
    } else {
        Err(ApiError::conflict(
            "not_in_dkg",
            format!(
                "key generation is not running (auction is {:?})",
                auction.status
            )
            .to_lowercase(),
        ))
    }
}

async fn system_transition(
    tx: &mut sqlx::PgConnection,
    auction: &AuctionRow,
    from: AuctionStatus,
    to: AuctionStatus,
) -> Result<(), ApiError> {
    debug_assert!(can_transition(from, to, Actor::System));
    let moved = sqlx::query(
        "UPDATE auctions SET status = $1, updated_at = now() WHERE id = $2 AND status = $3",
    )
    .bind(to)
    .bind(auction.id)
    .bind(from)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if moved == 0 {
        return Err(ApiError::conflict(
            "status_changed",
            "the auction changed; retry",
        ));
    }
    db::audit(
        &mut *tx,
        Some(auction.id),
        "system",
        None,
        "auction.transition",
        json!({ "from": from, "to": to }),
    )
    .await
}

/// `POST /v1/trustees/join`: claim a slot with a one-time invite code.
pub async fn join(
    State(state): State<AppState>,
    req: Signed,
) -> Result<Json<TrusteeJoined>, ApiError> {
    let input: JoinAsTrustee = req.json()?;
    let enc_pubkey = decode_hex::<32>(&input.enc_pubkey, "enc_pubkey")?;

    let mut tx = state.db.begin().await?;
    let slot: Option<(Uuid, Uuid, i16, String)> = sqlx::query_as(
        "SELECT t.id, t.auction_id, t.slot, t.label FROM trustees t JOIN auctions a ON a.id = t.auction_id \
         WHERE t.invite_code_hash = $1 AND t.pubkey IS NULL AND a.status = 'draft' FOR UPDATE OF t",
    )
    .bind(codes::sha256(&input.invite_code))
    .fetch_optional(&mut *tx)
    .await?;
    // Same error for unknown, used and expired invites.
    let (trustee_id, auction_id, slot, label) =
        slot.ok_or_else(|| ApiError::Unauthorized("invalid or already used invite code".into()))?;

    let joined = sqlx::query(
        "UPDATE trustees SET pubkey = $1, enc_pubkey = $2, joined_at = now() WHERE id = $3 \
         AND NOT EXISTS (SELECT 1 FROM trustees WHERE auction_id = $4 AND pubkey = $1)",
    )
    .bind(&req.pubkey[..])
    .bind(&enc_pubkey[..])
    .bind(trustee_id)
    .bind(auction_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if joined == 0 {
        return Err(ApiError::conflict(
            "already_trustee",
            "this key already holds a slot in this auction",
        ));
    }
    db::audit(
        &mut *tx,
        Some(auction_id),
        "trustee",
        Some(trustee_id),
        "trustee.joined",
        json!({ "slot": slot }),
    )
    .await?;

    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM trustees WHERE auction_id = $1 AND pubkey IS NULL",
    )
    .bind(auction_id)
    .fetch_one(&mut *tx)
    .await?;
    let code: String = sqlx::query_scalar("SELECT code FROM auctions WHERE id = $1")
        .bind(auction_id)
        .fetch_one(&mut *tx)
        .await?;
    if waiting == 0 {
        let auction = db::auction_by_code(&mut *tx, &code)
            .await?
            .ok_or(ApiError::NotFound)?;
        system_transition(&mut tx, &auction, AuctionStatus::Draft, AuctionStatus::Dkg).await?;
    }
    tx.commit().await?;

    Ok(Json(TrusteeJoined {
        auction_code: code,
        slot: slot as u16,
        label,
        min_signers: THRESHOLD.min_signers,
        max_signers: THRESHOLD.max_signers,
    }))
}

/// `GET /v1/auctions/{code}/dkg`
pub async fn state(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<DkgState>, ApiError> {
    let (auction, me, trustees) = as_trustee(&state, &req, &code).await?;
    let round1: Vec<PackageRow> = sqlx::query_as(
        "SELECT from_slot, to_slot, payload, signature FROM dkg_packages WHERE auction_id = $1 AND round = 1 ORDER BY from_slot",
    )
    .bind(auction.id)
    .fetch_all(&state.db)
    .await?;
    let round2: Vec<PackageRow> = sqlx::query_as(
        "SELECT from_slot, to_slot, payload, signature FROM dkg_packages \
         WHERE auction_id = $1 AND round = 2 AND to_slot = $2 ORDER BY from_slot",
    )
    .bind(auction.id)
    .bind(me.slot)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(DkgState {
        auction_code: auction.code.clone(),
        auction_status: auction.status,
        my_slot: me.slot as u16,
        min_signers: THRESHOLD.min_signers,
        max_signers: THRESHOLD.max_signers,
        trustees: trustees
            .into_iter()
            .map(|t| DkgTrustee {
                slot: t.slot as u16,
                label: t.label,
                pubkey: t.pubkey.map(hex::encode),
                enc_pubkey: t.enc_pubkey.map(hex::encode),
                dkg_status: t.dkg_status,
            })
            .collect(),
        round1: round1.into_iter().map(Into::into).collect(),
        round2_for_me: round2.into_iter().map(Into::into).collect(),
        escrow_ua: auction.escrow_ua,
    }))
}

async fn store_package(
    tx: &mut sqlx::PgConnection,
    auction_id: Uuid,
    round: i16,
    from_slot: i16,
    to_slot: Option<i16>,
    payload: &[u8],
    signature: &[u8; 64],
) -> Result<(), ApiError> {
    let inserted = sqlx::query(
        "INSERT INTO dkg_packages (auction_id, round, from_slot, to_slot, payload, signature) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
    )
    .bind(auction_id)
    .bind(round)
    .bind(from_slot)
    .bind(to_slot)
    .bind(payload)
    .bind(&signature[..])
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        // Idempotent retries are fine; a different package for the same slot is not.
        let existing: Vec<u8> = sqlx::query_scalar(
            "SELECT payload FROM dkg_packages WHERE auction_id = $1 AND round = $2 AND from_slot = $3 \
             AND to_slot IS NOT DISTINCT FROM $4",
        )
        .bind(auction_id)
        .bind(round)
        .bind(from_slot)
        .bind(to_slot)
        .fetch_one(&mut *tx)
        .await?;
        if existing != payload {
            return Err(ApiError::conflict(
                "package_exists",
                "a different package was already submitted",
            ));
        }
    }
    Ok(())
}

/// `POST /v1/auctions/{code}/dkg/round1`
pub async fn round1(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<DkgState>, ApiError> {
    let (auction, me, _) = as_trustee(&state, &req, &code).await?;
    require_dkg(&auction)?;
    let input: PostRound1 = req.json()?;
    let payload = decode_payload(&input.payload)?;
    let signature = decode_hex::<64>(&input.signature, "signature")?;
    if !verify_dkg_package(
        &req.pubkey,
        &signature,
        &auction.code,
        1,
        me.slot as u16,
        None,
        &payload,
    ) {
        return Err(ApiError::BadRequest(
            "package signature does not verify".into(),
        ));
    }

    let mut tx = state.db.begin().await?;
    store_package(&mut tx, auction.id, 1, me.slot, None, &payload, &signature).await?;
    sqlx::query(
        "UPDATE trustees SET dkg_status = 'round1' WHERE id = $1 AND dkg_status = 'invited'",
    )
    .bind(me.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    self::state(State(state), Path(code), req).await
}

/// `POST /v1/auctions/{code}/dkg/round2`: one sealed package per other trustee.
pub async fn round2(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<DkgState>, ApiError> {
    let (auction, me, trustees) = as_trustee(&state, &req, &code).await?;
    require_dkg(&auction)?;
    let round1_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM dkg_packages WHERE auction_id = $1 AND round = 1")
            .bind(auction.id)
            .fetch_one(&state.db)
            .await?;
    if round1_count != trustees.len() as i64 {
        return Err(ApiError::conflict(
            "round1_incomplete",
            "waiting for every trustee's round 1 package",
        ));
    }

    let input: PostRound2 = req.json()?;
    let expected: BTreeSet<u16> = trustees
        .iter()
        .map(|t| t.slot as u16)
        .filter(|s| *s != me.slot as u16)
        .collect();
    let got: BTreeSet<u16> = input.packages.iter().map(|p| p.to_slot).collect();
    if got != expected || input.packages.len() != expected.len() {
        return Err(ApiError::BadRequest(
            "send exactly one round 2 package to each other trustee".into(),
        ));
    }

    let mut tx = state.db.begin().await?;
    for package in &input.packages {
        let payload = decode_payload(&package.payload)?;
        let signature = decode_hex::<64>(&package.signature, "signature")?;
        if !verify_dkg_package(
            &req.pubkey,
            &signature,
            &auction.code,
            2,
            me.slot as u16,
            Some(package.to_slot),
            &payload,
        ) {
            return Err(ApiError::BadRequest(
                "package signature does not verify".into(),
            ));
        }
        store_package(
            &mut tx,
            auction.id,
            2,
            me.slot,
            Some(package.to_slot as i16),
            &payload,
            &signature,
        )
        .await?;
    }
    sqlx::query("UPDATE trustees SET dkg_status = 'round2' WHERE id = $1 AND dkg_status IN ('invited', 'round1')")
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    self::state(State(state), Path(code), req).await
}

/// `POST /v1/auctions/{code}/dkg/complete`: report the derived public key package.
/// The last of the three reports derives the escrow and makes the auction ready.
pub async fn complete(
    State(state): State<AppState>,
    Path(code): Path<String>,
    req: Signed,
) -> Result<Json<DkgState>, ApiError> {
    let (auction, me, _) = as_trustee(&state, &req, &code).await?;
    require_dkg(&auction)?;
    let input: CompleteDkg = req.json()?;
    let reported = decode_payload(&input.public_key_package)?;
    PublicKeyPackage::deserialize(&reported)
        .map_err(|_| ApiError::BadRequest("invalid public key package".into()))?;
    let data_key = state
        .settings
        .data_key
        .clone()
        .ok_or_else(|| ApiError::Unavailable("SF_DATA_KEY is not configured".into()))?;

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE trustees SET dkg_status = 'complete', public_key_package = $1 WHERE id = $2",
    )
    .bind(&reported)
    .bind(me.id)
    .execute(&mut *tx)
    .await?;

    let packages: Vec<Option<Vec<u8>>> = sqlx::query_scalar(
        "SELECT public_key_package FROM trustees WHERE auction_id = $1 FOR UPDATE",
    )
    .bind(auction.id)
    .fetch_all(&mut *tx)
    .await?;
    if packages.iter().all(Option::is_some) {
        let distinct: BTreeSet<&Vec<u8>> = packages.iter().flatten().collect();
        if distinct.len() != 1 {
            db::audit(
                &mut *tx,
                Some(auction.id),
                "system",
                None,
                "dkg.mismatch",
                json!({}),
            )
            .await?;
            tx.commit().await?;
            return Err(ApiError::conflict(
                "dkg_mismatch",
                "trustees derived different group keys; the DKG must be restarted",
            ));
        }
        let pubkeys =
            PublicKeyPackage::deserialize(&reported).map_err(|e| ApiError::Internal(e.into()))?;
        let ak = frost_dkg::group_key_bytes(&pubkeys).map_err(|e| ApiError::Internal(e.into()))?;
        let fvk = address::escrow_fvk(&ak, &mut OsRng).map_err(|e| ApiError::Internal(e.into()))?;
        let params = validate::params_for(auction.network);
        let escrow_ua =
            address::escrow_address(&fvk, &params).map_err(|e| ApiError::Internal(e.into()))?;
        let (ufvk, _uivk) =
            address::escrow_view_keys(&fvk, &params).map_err(|e| ApiError::Internal(e.into()))?;

        sqlx::query(
            "UPDATE auctions SET escrow_ua = $1, escrow_ufvk_enc = pgp_sym_encrypt($2, $3), escrow_group_key = $4, \
             escrow_birthday_height = $6, updated_at = now() WHERE id = $5",
        )
        .bind(&escrow_ua)
        .bind(&ufvk)
        .bind(&data_key)
        .bind(&ak[..])
        .bind(auction.id)
        .bind(state.chain_tip().map(|h| h as i32))
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM dkg_packages WHERE auction_id = $1")
            .bind(auction.id)
            .execute(&mut *tx)
            .await?;
        system_transition(&mut tx, &auction, AuctionStatus::Dkg, AuctionStatus::Ready).await?;
    }
    tx.commit().await?;
    self::state(State(state), Path(code), req).await
}
