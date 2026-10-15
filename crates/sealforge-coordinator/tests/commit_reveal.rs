//! Task 4.6: a commit-reveal auction from deposits and reveals to its result.

use std::collections::BTreeMap;

use sealforge_chain::ReceivedPayment;
use sealforge_coordinator::{closer, db, scanner, AppState, Settings};
use sealforge_core::{
    memo::{BidMemo, RevealMemo},
    receipt::price_commitment,
};
use sqlx::PgPool;

const CODE: &str = "CRTEST0001";
const KEY: &str = "test-data-key";
const REFUND_UA: &str = "uregtest1vx67p0383w4xq367fpfdpcztgll39hxyp3rmk67ggkhfldza0pqzmfscrddtnfccq20qu8x3pgrcjxd3n3gvx3smepwhy8prhcvpwq4y";
const ZEC: u64 = 100_000_000;
const REVEAL_FEE: u64 = 10_000;

/// (txid, status, invalid_reason, reveal_price_zat, is_winner, price_paid_zat, refund_due_zat)
type Row = (Vec<u8>, String, Option<String>, Option<i64>, bool, i64, i64);

struct Bidder {
    tx: u8,
    deposit: u64,
    price: u64,
}

impl Bidder {
    fn salt(&self) -> [u8; 32] {
        [self.tx; 32]
    }
    fn commitment(&self) -> [u8; 32] {
        price_commitment(CODE, REFUND_UA, self.price, &self.salt())
    }
    fn deposit_payment(&self, height: u32) -> ReceivedPayment {
        ReceivedPayment {
            txid: [self.tx; 32],
            output_index: 0,
            block_height: height,
            amount_zat: self.deposit,
            memo: Some(
                BidMemo {
                    auction_code: CODE.into(),
                    commitment: self.commitment(),
                    refund_ua: REFUND_UA.into(),
                }
                .encode()
                .unwrap(),
            ),
        }
    }
    fn reveal_payment(&self, tx: u8, height: u32, price: u64) -> ReceivedPayment {
        ReceivedPayment {
            txid: [tx; 32],
            output_index: 0,
            block_height: height,
            amount_zat: REVEAL_FEE,
            memo: Some(
                RevealMemo {
                    auction_code: CODE.into(),
                    commitment: self.commitment(),
                    price_zat: price,
                    salt: self.salt(),
                }
                .encode()
                .unwrap(),
            ),
        }
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn deposits_are_budgets_and_reveals_set_prices(db: PgPool) {
    let state = AppState::new(
        db.clone(),
        Settings {
            invite_codes: vec![],
            data_key: Some(KEY.into()),
        },
    );
    sqlx::query("INSERT INTO organisers (display_name, pubkey) VALUES ('Org', $1)")
        .bind(&[3u8; 32][..])
        .execute(&db)
        .await
        .unwrap();
    // Bidding 100..=200, reveals 201..=344, 2 lots, uniform price, reserve 0.01 ZEC.
    sqlx::query(
        "INSERT INTO auctions (code, organiser_id, title, network, mode, bid_mode, quantity, reserve_zat, \
         open_height, close_height, reveal_close_height, payout_ua, status) \
         SELECT $1, id, 'CR', 'regtest', 'uniform_price', 'commit_reveal', 2, 1000000, 100, 200, 344, $2, 'open' \
         FROM organisers",
    )
    .bind(CODE)
    .bind(REFUND_UA)
    .execute(&db)
    .await
    .unwrap();
    let auction = db::auction_by_code(&db, CODE).await.unwrap().unwrap();

    let a = Bidder {
        tx: 1,
        deposit: ZEC,
        price: 42_000_000,
    };
    let b = Bidder {
        tx: 2,
        deposit: ZEC / 2,
        price: 30_000_000,
    };
    let c = Bidder {
        tx: 3,
        deposit: ZEC / 2,
        price: 10_000_000,
    };
    let d = Bidder {
        tx: 4,
        deposit: ZEC / 5,
        price: 15_000_000,
    }; // never reveals

    // While bidding is open only deposits exist; nobody sees a price.
    let deposits = [
        a.deposit_payment(150),
        b.deposit_payment(151),
        c.deposit_payment(152),
        d.deposit_payment(153),
    ];
    scanner::ingest(&state, &auction, &deposits).await.unwrap();

    let reveals = [
        b.reveal_payment(20, 199, b.price), // before the close: doesn't count
        a.reveal_payment(21, 250, a.price),
        b.reveal_payment(22, 251, b.price),
        c.reveal_payment(23, 252, c.price + 1), // wrong price: doesn't open
        c.reveal_payment(24, 253, c.price),
    ];
    scanner::ingest(&state, &auction, &reveals).await.unwrap();

    // Not due until the reveal window has 10 confirmations.
    let at = |h: i32| {
        sqlx::query(
            "INSERT INTO scanner_state (auction_id, last_height, last_hash) VALUES ($1, $2, '\\x00') \
             ON CONFLICT (auction_id) DO UPDATE SET last_height = $2",
        )
        .bind(auction.id)
        .bind(h)
        .execute(&db)
    };
    at(260).await.unwrap();
    assert!(!closer::close_auction(&state, &auction).await.unwrap());
    at(354).await.unwrap();
    assert!(closer::close_auction(&state, &auction).await.unwrap());

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT txid, status::text, invalid_reason, reveal_price_zat, coalesce(is_winner, false), \
         coalesce(price_paid_zat, 0), coalesce(refund_due_zat, 0) FROM bids ORDER BY txid",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    let by_tx: BTreeMap<u8, _> = rows.into_iter().map(|r| (r.0[0], r)).collect();
    let paid = |tx: u8| by_tx[&tx].5 as u64;
    let refund = |tx: u8| by_tx[&tx].6 as u64;

    // Uniform price over revealed prices: A (0.42) and B (0.30) win at 0.30.
    assert!(by_tx[&1].4);
    assert_eq!(by_tx[&1].3, Some(42_000_000));
    assert_eq!(paid(1), 30_000_000);
    assert_eq!(
        refund(1),
        ZEC + REVEAL_FEE - 30_000_000,
        "deposit + reveal - price"
    );
    assert_eq!(paid(2), 30_000_000);
    assert_eq!(refund(2), ZEC / 2 + REVEAL_FEE - 30_000_000);
    // C revealed below the winners: loses, everything back.
    assert_eq!((by_tx[&3].1.as_str(), by_tx[&3].4), ("valid", false));
    assert_eq!(refund(3), ZEC / 2 + REVEAL_FEE);
    // D never revealed: refunded in full, not in the result.
    assert_eq!(by_tx[&4].2.as_deref(), Some("not_revealed"));
    assert_eq!(refund(4), ZEC / 5);
    // Counted reveals are folded into their deposit's refund; the rest need review.
    assert_eq!(refund(21), 0);
    assert_eq!(refund(22), 0);
    assert_eq!(refund(24), 0);
    assert_eq!(refund(20), REVEAL_FEE, "early reveal didn't count");
    assert_eq!(refund(23), REVEAL_FEE, "wrong price didn't count");

    let (price, winners, scan_to): (Option<i64>, i32, i32) = sqlx::query_as(
        "SELECT clearing_price_zat, winner_count, scan_to_height FROM clearing_results",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((price, winners, scan_to), (Some(30_000_000), 2, 344));
}
