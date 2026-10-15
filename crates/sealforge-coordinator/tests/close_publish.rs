//! Task 3.3: close at the close height, then publish (PRD AC3, AC4, AC5, AC6).

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use rand::rngs::OsRng;
use sealforge_api::{auth::sign_request, types::PublishedResult};
use sealforge_coordinator::{closer, refunds, routes, AppState, Settings};
use sealforge_core::{
    clearing::{AuctionMode, AuctionRules},
    refunds::RefundPolicy,
    result::{compute_result, ScannedBid},
    Zatoshi,
};
use sealforge_escrow::{address, dkg, Threshold};
use serde_json::{json, Value};
use sqlx::PgPool;
use zcash_protocol::consensus::TEST_NETWORK;

const DATA_KEY: &str = "test-data-key";
const PAYOUT_UA: &str = "utest10wa7f6qs6jevt8pl9vezqtrnm9qdkcccdyjtsam4sqgyq4mwvnvxaen2nnsd0yu47qh6a6s0payftzz4h86c6caev4qguam3huf3z7yu";
/// (txid, status, invalid_reason, is_winner, price_paid_zat, refund_due_zat) as stored.
type StoredBid = (Vec<u8>, String, Option<String>, Option<bool>, i64, i64);
/// The same, with txid reduced to its first byte for readable assertions.
type Outcome<'a> = (u8, &'a str, Option<&'a str>, bool, i64, i64);

const OPEN: u32 = 3_900_000;
const CLOSE: u32 = 4_000_000;

async fn call(
    app: &Router,
    key: Option<&SigningKey>,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    use tower::ServiceExt;
    let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    let mut req = Request::builder()
        .method(method.clone())
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = key {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        req = req.header(
            header::AUTHORIZATION,
            sign_request(key, method.as_str(), path, ts, &rand::random(), &bytes),
        );
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

struct Bid {
    tx: u8,
    height: u32,
    amount: u64,
    commitment: u8,
    /// Memo-level problem recorded by the scanner, if any.
    scanner_reason: Option<&'static str>,
}

async fn insert_bid(db: &PgPool, code: &str, b: &Bid) {
    sqlx::query(
        "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, memo_version, \
         commitment, status, invalid_reason) \
         SELECT id, $2, 0, 'ironwood', $3, now(), $4, 1, $5, CASE WHEN $6::text IS NULL THEN 'valid' ELSE 'invalid' END::bid_status, $6 \
         FROM auctions WHERE code = $1",
    )
    .bind(code)
    .bind(&[b.tx; 32][..])
    .bind(b.height as i32)
    .bind(b.amount as i64)
    .bind(&[b.commitment; 32][..])
    .bind(b.scanner_reason)
    .execute(db)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn closes_at_close_height_and_publishes_an_auditable_result(db: PgPool) {
    let state = AppState::new(
        db.clone(),
        Settings {
            invite_codes: vec!["inv".into()],
            data_key: Some(DATA_KEY.into()),
        },
    );
    let app = routes::router(state.clone());
    let org = SigningKey::from_bytes(&[1; 32]);
    assert_eq!(
        call(
            &app,
            Some(&org),
            Method::POST,
            "/v1/organisers",
            Some(json!({ "display_name": "Org", "invite_code": "inv" }))
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (status, created) = call(
        &app,
        Some(&org),
        Method::POST,
        "/v1/auctions",
        Some(json!({
            "title": "Two lots", "network": "testnet", "mode": "uniform_price", "quantity": 2,
            "reserve_zat": 1_000_000, "close_height": CLOSE, "payout_ua": PAYOUT_UA,
            "trustees": [{ "slot": 1, "label": "A" }, { "slot": 2, "label": "B" }, { "slot": 3, "label": "C" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let code = created["auction"]["code"].as_str().unwrap().to_owned();

    // Stand-in for the DKG (task 2.4 covers it end to end): a real escrow with an encrypted UFVK.
    let (_, pubkeys) = dkg::run_local(Threshold::default(), &mut OsRng).unwrap();
    let fvk = address::escrow_fvk(&dkg::group_key_bytes(&pubkeys).unwrap(), &mut OsRng).unwrap();
    let ua = address::escrow_address(&fvk, &TEST_NETWORK).unwrap();
    let (ufvk, expected_uivk) = address::escrow_view_keys(&fvk, &TEST_NETWORK).unwrap();
    sqlx::query("UPDATE auctions SET status = 'ready', escrow_ua = $1, escrow_ufvk_enc = pgp_sym_encrypt($2, $3) WHERE code = $4")
        .bind(&ua)
        .bind(&ufvk)
        .bind(DATA_KEY)
        .bind(&code)
        .execute(&db)
        .await
        .unwrap();
    state.set_chain_tip(OPEN);
    assert_eq!(
        call(
            &app,
            Some(&org),
            Method::POST,
            &format!("/v1/auctions/{code}/open"),
            None
        )
        .await
        .0,
        StatusCode::OK
    );

    let bids = [
        Bid {
            tx: 1,
            height: OPEN + 10,
            amount: 5_000_000,
            commitment: 1,
            scanner_reason: None,
        }, // A: wins
        Bid {
            tx: 2,
            height: OPEN + 50_000,
            amount: 3_000_000,
            commitment: 2,
            scanner_reason: None,
        }, // B: loses
        Bid {
            tx: 3,
            height: OPEN + 60_000,
            amount: 4_000_000,
            commitment: 3,
            scanner_reason: None,
        }, // C: wins, sets price
        Bid {
            tx: 4,
            height: OPEN + 70_000,
            amount: 500_000,
            commitment: 4,
            scanner_reason: None,
        }, // D: under reserve
        Bid {
            tx: 5,
            height: CLOSE + 1,
            amount: 9_000_000,
            commitment: 5,
            scanner_reason: None,
        }, // E: late
        Bid {
            tx: 6,
            height: OPEN + 80_000,
            amount: 7_000_000,
            commitment: 1,
            scanner_reason: None,
        }, // F: duplicate of A
        Bid {
            tx: 7,
            height: OPEN + 90_000,
            amount: 2_000_000,
            commitment: 6,
            scanner_reason: Some("wrong_auction"),
        }, // G
        Bid {
            tx: 8,
            height: OPEN - 1,
            amount: 6_000_000,
            commitment: 7,
            scanner_reason: None,
        }, // H: before open
    ];
    for b in &bids {
        insert_bid(&db, &code, b).await;
    }

    // Not due until the scanner is CLOSE_CONFIRMATIONS past the close height.
    assert!(
        closer::close_due(&state).await.unwrap().is_empty(),
        "no scanner progress yet"
    );
    let auction_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM auctions WHERE code = $1")
        .bind(&code)
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO scanner_state (auction_id, last_height, last_hash) VALUES ($1, $2, '\\x00')",
    )
    .bind(auction_id)
    .bind((CLOSE + closer::CLOSE_CONFIRMATIONS - 1) as i32)
    .execute(&db)
    .await
    .unwrap();
    assert!(
        closer::close_due(&state).await.unwrap().is_empty(),
        "one confirmation short"
    );
    sqlx::query("UPDATE scanner_state SET last_height = $2 WHERE auction_id = $1")
        .bind(auction_id)
        .bind((CLOSE + closer::CLOSE_CONFIRMATIONS) as i32)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(closer::close_due(&state).await.unwrap(), vec![code.clone()]);

    // Every payment is classified and has its refund set.
    let rows: Vec<StoredBid> = sqlx::query_as(
        "SELECT txid, status::text, invalid_reason, is_winner, price_paid_zat, refund_due_zat FROM bids ORDER BY txid",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    let got: Vec<Outcome> = rows
        .iter()
        .map(|(tx, s, r, w, p, d)| (tx[0], s.as_str(), r.as_deref(), w.unwrap_or(false), *p, *d))
        .collect();
    assert_eq!(
        got,
        vec![
            (1, "valid", None, true, 4_000_000, 1_000_000),
            (2, "valid", None, false, 0, 3_000_000),
            (3, "valid", None, true, 4_000_000, 0),
            (4, "under_reserve", None, false, 0, 500_000),
            (5, "late", None, false, 0, 9_000_000),
            (
                6,
                "invalid",
                Some("duplicate_commitment"),
                false,
                0,
                7_000_000
            ),
            (7, "invalid", Some("wrong_auction"), false, 0, 2_000_000),
            (8, "invalid", Some("before_open"), false, 0, 6_000_000),
        ]
    );

    // Not public until published.
    assert_eq!(
        call(
            &app,
            None,
            Method::GET,
            &format!("/v1/auctions/{code}/result"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, body) = call(
        &app,
        Some(&org),
        Method::POST,
        &format!("/v1/auctions/{code}/publish"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "published");
    assert_eq!(
        call(
            &app,
            Some(&org),
            Method::POST,
            &format!("/v1/auctions/{code}/publish"),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let (status, body) = call(
        &app,
        None,
        Method::GET,
        &format!("/v1/auctions/{code}/result"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let published: PublishedResult = serde_json::from_value(body).unwrap();
    assert_eq!(published.uivk, expected_uivk);
    assert_eq!(
        (published.scan_from_height, published.scan_to_height),
        (OPEN, CLOSE)
    );
    assert_eq!(published.clearing_price_zat, Some(4_000_000));
    assert_eq!(published.total_volume_zat, 12_000_000);
    assert_eq!((published.valid_bid_count, published.winner_count), (3, 2));
    assert_eq!(
        published.bids.len(),
        4,
        "valid + under-reserve bids are in the tree"
    );

    // An auditor recomputing from the included bids alone gets the same root.
    let included: Vec<ScannedBid> = bids[..4]
        .iter()
        .map(|b| ScannedBid {
            txid: [b.tx; 32],
            output_index: 0,
            block_height: b.height,
            amount: Zatoshi(b.amount),
            commitment: [b.commitment; 32],
        })
        .collect();
    let rules = AuctionRules {
        mode: AuctionMode::UniformPrice,
        quantity: 2,
        reserve: Zatoshi(1_000_000),
    };
    let recomputed = compute_result(&rules, &included).unwrap();
    assert_eq!(published.merkle_root, hex::encode(recomputed.merkle_root));

    // ---- Refund planning (task 3.9). The scanner stores each memo's refund
    // address; bid G (wrong auction) stands in for one we couldn't read.
    sqlx::query(
        "UPDATE bids SET refund_ua_enc = pgp_sym_encrypt('utest1refund', $1) WHERE txid <> $2",
    )
    .bind(DATA_KEY)
    .bind(&[7u8; 32][..])
    .execute(&db)
    .await
    .unwrap();

    let policy = RefundPolicy::default();
    let before = chrono::Utc::now();
    assert_eq!(
        refunds::plan_published(&state, &policy).await.unwrap(),
        vec![code.clone()]
    );
    assert!(
        refunds::plan_published(&state, &policy)
            .await
            .unwrap()
            .is_empty(),
        "planned once"
    );
    let status: String = sqlx::query_scalar("SELECT status::text FROM auctions WHERE code = $1")
        .bind(&code)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "refunding");

    // Every planned bid's notes add up to its refund (minus dust), within the delay window.
    let sums: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT b.refund_due_zat, sum(n.denomination_zat)::bigint FROM refund_notes n JOIN bids b ON b.id = n.bid_id \
         GROUP BY b.id, b.refund_due_zat",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        sums.len(),
        6,
        "every refundable bid except the unreadable one (C won with no refund)"
    );
    for (due, total) in &sums {
        assert_eq!(*total, due - due % 10_000);
    }
    let (min_at, max_at): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT min(not_before), max(not_before) FROM refund_notes")
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(min_at >= before + chrono::Duration::seconds(policy.min_delay_secs as i64));
    assert!(max_at <= chrono::Utc::now() + chrono::Duration::seconds(policy.max_delay_secs as i64));

    let (status, progress) = call(
        &app,
        None,
        Method::GET,
        &format!("/v1/auctions/{code}/refunds"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(progress["needs_manual_review"], 1);
    assert_eq!(progress["notes_planned"], progress["notes_total"]);

    // Nothing is due yet; five hours later everything is, in batches of at most 20.
    assert_eq!(
        refunds::batch_due(&state, &policy, chrono::Utc::now())
            .await
            .unwrap(),
        0
    );
    // When the first note falls due, it is held for more bidders rather than sent alone.
    let first: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT min(not_before) FROM refund_notes")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        refunds::batch_due(&state, &policy, first + chrono::Duration::seconds(1))
            .await
            .unwrap(),
        0,
        "a lone bidder's note waits to share a batch"
    );
    let later = chrono::Utc::now() + chrono::Duration::hours(5);
    assert!(refunds::batch_due(&state, &policy, later).await.unwrap() > 0);
    let unbatched: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refund_notes WHERE status <> 'batched'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(unbatched, 0);
    let largest: i64 = sqlx::query_scalar(
        "SELECT max(c) FROM (SELECT count(*) AS c FROM refund_notes GROUP BY batch_id) x",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(largest <= policy.max_outputs_per_batch as i64);

    // A payment that lands after the result is planned for a full refund.
    let (auction_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM auctions WHERE code = $1")
        .bind(&code)
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, status, \
         refund_due_zat, is_winner, refund_ua_enc) \
         VALUES ($1, decode(repeat('ee', 32), 'hex'), 0, 'ironwood', 999999, now(), 25000000, 'late', 25000000, false, pgp_sym_encrypt('utest1late', $2))",
    )
    .bind(auction_id)
    .bind(DATA_KEY)
    .execute(&db)
    .await
    .unwrap();
    assert!(refunds::plan_late(&state, &policy).await.unwrap() > 0);
    assert_eq!(
        refunds::plan_late(&state, &policy).await.unwrap(),
        0,
        "planned once"
    );
    let late_total: i64 = sqlx::query_scalar(
        "SELECT sum(n.denomination_zat)::bigint FROM refund_notes n JOIN bids b ON b.id = n.bid_id WHERE b.txid = decode(repeat('ee', 32), 'hex')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(late_total, 25_000_000);
}
