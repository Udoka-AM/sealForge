//! Task 2.5: recording escrow payments found by the scanner.

use sealforge_api::types::AuctionStatus;
use sealforge_chain::ReceivedPayment;
use sealforge_coordinator::{db, scanner, AppState, Settings};
use sealforge_core::{memo::BidMemo, receipt::commitment};
use sqlx::PgPool;

const CODE: &str = "ABCDEFGHJK";
const KEY: &str = "test-data-key";

/// (txid, status, invalid_reason, commitment, decrypted refund address)
type StoredBid = (
    Vec<u8>,
    String,
    Option<String>,
    Option<Vec<u8>>,
    Option<String>,
);
/// The same, reduced for readable assertions.
type Summary<'a> = (u8, &'a str, Option<&'a str>, bool, Option<&'a str>);

fn payment(tx: u8, memo: Option<String>) -> ReceivedPayment {
    ReceivedPayment {
        txid: [tx; 32],
        output_index: 0,
        block_height: 1000 + u32::from(tx),
        amount_zat: 5_000_000,
        memo,
    }
}

/// A real regtest Unified Address with an Orchard receiver.
const REFUND_UA: &str = "uregtest1vx67p0383w4xq367fpfdpcztgll39hxyp3rmk67ggkhfldza0pqzmfscrddtnfccq20qu8x3pgrcjxd3n3gvx3smepwhy8prhcvpwq4y";

fn bid_memo(code: &str) -> String {
    memo_with_refund(code, REFUND_UA)
}

fn memo_with_refund(code: &str, refund_ua: &str) -> String {
    BidMemo {
        auction_code: code.into(),
        commitment: commitment(code, refund_ua, &[1; 32]),
        refund_ua: refund_ua.into(),
    }
    .encode()
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn records_payments_with_memo_level_checks(db: PgPool) {
    let state = AppState::new(
        db.clone(),
        Settings {
            invite_codes: vec![],
            data_key: Some(KEY.into()),
        },
    );
    sqlx::query("INSERT INTO organisers (display_name, pubkey) VALUES ('Org', $1)")
        .bind(&[9u8; 32][..])
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO auctions (code, organiser_id, title, network, close_height, payout_ua, status) \
         SELECT $1, id, 'T', 'regtest', 2000, 'uregtest1payout', 'open' FROM organisers",
    )
    .bind(CODE)
    .execute(&db)
    .await
    .unwrap();
    let auction = db::auction_by_code(&db, CODE).await.unwrap().unwrap();
    assert_eq!(auction.status, AuctionStatus::Open);

    let payments = vec![
        payment(1, Some(bid_memo(CODE))),
        payment(2, Some(bid_memo("ZZZZZZZZZZ"))),
        payment(3, None),
        payment(4, Some("thanks for the auction".into())),
    ];
    assert_eq!(
        scanner::ingest(&state, &auction, &payments).await.unwrap(),
        4
    );
    assert_eq!(
        scanner::ingest(&state, &auction, &payments).await.unwrap(),
        0,
        "re-scans are idempotent"
    );

    let rows: Vec<StoredBid> = sqlx::query_as(
        "SELECT txid, status::text, invalid_reason, commitment, pgp_sym_decrypt(refund_ua_enc, $1) \
         FROM bids ORDER BY txid",
    )
    .bind(KEY)
    .fetch_all(&db)
    .await
    .unwrap();
    let summary: Vec<Summary> = rows
        .iter()
        .map(|(tx, s, r, c, ua)| (tx[0], s.as_str(), r.as_deref(), c.is_some(), ua.as_deref()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (1, "valid", None, true, Some(REFUND_UA)),
            (2, "invalid", Some("wrong_auction"), true, Some(REFUND_UA)),
            (3, "invalid", Some("bad_memo"), false, None),
            (4, "invalid", Some("bad_memo"), false, None),
        ]
    );

    // Refund addresses that can't be paid privately aren't stored: those bids go to
    // manual review instead of breaking a refund batch.
    let unusable = [
        payment(7, Some(memo_with_refund(CODE, "uregtest1refund"))),
        payment(8, Some(memo_with_refund(CODE, "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd"))),
        payment(
            9,
            Some(memo_with_refund(
                CODE,
                "utest10wa7f6qs6jevt8pl9vezqtrnm9qdkcccdyjtsam4sqgyq4mwvnvxaen2nnsd0yu47qh6a6s0payftzz4h86c6caev4qguam3huf3z7yu",
            )),
        ),
    ];
    assert_eq!(
        scanner::ingest(&state, &auction, &unusable).await.unwrap(),
        3
    );
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM bids WHERE txid = ANY($1) AND status = 'valid' AND refund_ua_enc IS NULL",
    )
    .bind(unusable.iter().map(|p| p.txid.to_vec()).collect::<Vec<_>>())
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(stored, 3, "valid bids, no refund address");

    // The public bid count appears only from 5 bids; the owner always sees it.
    let auction = db::auction_by_code(&db, CODE).await.unwrap().unwrap();
    assert_eq!(
        db::auction_view(&db, &auction, false)
            .await
            .unwrap()
            .bid_count,
        Some(7)
    );
    sqlx::query("DELETE FROM bids WHERE txid = ANY($1)")
        .bind(unusable.iter().map(|p| p.txid.to_vec()).collect::<Vec<_>>())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        db::auction_view(&db, &auction, false)
            .await
            .unwrap()
            .bid_count,
        None
    );
    assert_eq!(
        db::auction_view(&db, &auction, true)
            .await
            .unwrap()
            .bid_count,
        Some(4)
    );

    // After close the result is fixed: a new payment is late and refunded in full.
    sqlx::query("UPDATE auctions SET status = 'refunding'")
        .execute(&db)
        .await
        .unwrap();
    let auction = db::auction_by_code(&db, CODE).await.unwrap().unwrap();
    assert_eq!(
        scanner::ingest(&state, &auction, &[payment(5, Some(bid_memo(CODE)))])
            .await
            .unwrap(),
        1
    );
    let late: (String, Option<i64>, i64, Option<bool>) = sqlx::query_as(
        "SELECT status::text, refund_due_zat, amount_zat, is_winner FROM bids WHERE txid = $1",
    )
    .bind(&payment(5, None).txid[..])
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(late.0, "late");
    assert_eq!(late.1, Some(late.2));
    assert_eq!(late.3, Some(false));

    // The organiser's fee buffer is recorded as such: never refunded, never a bid.
    let fee = payment(6, Some(format!("SF1FEE:{CODE}")));
    assert_eq!(
        scanner::ingest(&state, &auction, std::slice::from_ref(&fee))
            .await
            .unwrap(),
        1
    );
    scanner::ingest(&state, &auction, &[fee]).await.unwrap();
    let (reason, due): (Option<String>, Option<i64>) =
        sqlx::query_as("SELECT invalid_reason, refund_due_zat FROM bids WHERE txid = $1")
            .bind(&payment(6, None).txid[..])
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("fee_buffer"));
    assert_eq!(due, Some(0));
    let buffer: i64 = sqlx::query_scalar("SELECT fee_buffer_zat FROM auctions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(buffer, payment(6, None).amount_zat as i64, "counted once");
}
