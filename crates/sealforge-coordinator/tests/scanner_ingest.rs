//! Task 2.5: recording escrow payments found by the scanner.

use sealforge_api::types::AuctionStatus;
use sealforge_chain::ReceivedPayment;
use sealforge_coordinator::{db, scanner, AppState, Settings};
use sealforge_core::{memo::BidMemo, receipt::commitment};
use sqlx::PgPool;

const CODE: &str = "ABCDEFGHJK";
const KEY: &str = "test-data-key";

fn payment(tx: u8, memo: Option<String>) -> ReceivedPayment {
    ReceivedPayment {
        txid: [tx; 32],
        output_index: 0,
        block_height: 1000 + u32::from(tx),
        amount_zat: 5_000_000,
        memo,
    }
}

fn bid_memo(code: &str) -> String {
    BidMemo {
        auction_code: code.into(),
        commitment: commitment(code, "uregtest1refund", &[1; 32]),
        refund_ua: "uregtest1refund".into(),
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

    let rows: Vec<(Vec<u8>, String, Option<String>, Option<Vec<u8>>, Option<String>)> = sqlx::query_as(
        "SELECT txid, status::text, invalid_reason, commitment, pgp_sym_decrypt(refund_ua_enc, $1) \
         FROM bids ORDER BY txid",
    )
    .bind(KEY)
    .fetch_all(&db)
    .await
    .unwrap();
    let summary: Vec<(u8, &str, Option<&str>, bool, Option<&str>)> = rows
        .iter()
        .map(|(tx, s, r, c, ua)| (tx[0], s.as_str(), r.as_deref(), c.is_some(), ua.as_deref()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (1, "valid", None, true, Some("uregtest1refund")),
            (
                2,
                "invalid",
                Some("wrong_auction"),
                true,
                Some("uregtest1refund")
            ),
            (3, "invalid", Some("bad_memo"), false, None),
            (4, "invalid", Some("bad_memo"), false, None),
        ]
    );
}
