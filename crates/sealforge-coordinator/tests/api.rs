//! API integration tests against a real Postgres (each test gets a fresh
//! database via `sqlx::test`). Run with DATABASE_URL pointing at a server the
//! user can create databases on, e.g. the docker compose Postgres.

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use sealforge_api::{
    auth::sign_request,
    types::{AuctionStatus, AuctionView, CreatedAuction},
};
use sealforge_coordinator::{routes, AppState, Settings};
use serde_json::{json, Value};
use sqlx::PgPool;

const INVITE: &str = "test-invite";
/// A testnet UA with an Orchard receiver (throwaway key).
const TESTNET_UA: &str = "utest10wa7f6qs6jevt8pl9vezqtrnm9qdkcccdyjtsam4sqgyq4mwvnvxaen2nnsd0yu47qh6a6s0payftzz4h86c6caev4qguam3huf3z7yu";

struct Api {
    app: Router,
    state: AppState,
}

impl Api {
    fn new(db: PgPool) -> Self {
        let state = AppState::new(
            db,
            Settings {
                invite_codes: vec![INVITE.into()],
                data_key: Some("test-data-key".into()),
            },
        );
        Self {
            app: routes::router(state.clone()),
            state,
        }
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, Value) {
        use tower::ServiceExt;
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, body)
    }

    fn signed_at(
        key: &SigningKey,
        method: Method,
        path: &str,
        body: Option<Value>,
        ts: i64,
        nonce: [u8; 16],
    ) -> Request<Body> {
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        let auth = sign_request(key, method.as_str(), path, ts, &nonce, &bytes);
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, auth)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes))
            .unwrap()
    }

    async fn signed(
        &self,
        key: &SigningKey,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let nonce: [u8; 16] = rand::random();
        self.send(Self::signed_at(key, method, path, body, now, nonce))
            .await
    }

    async fn public(&self, path: &str) -> (StatusCode, Value) {
        self.send(Request::get(path).body(Body::empty()).unwrap())
            .await
    }

    async fn register(&self, key: &SigningKey) {
        let (status, _) = self
            .signed(
                key,
                Method::POST,
                "/v1/organisers",
                Some(json!({ "display_name": "Org", "invite_code": INVITE })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    async fn create_auction(&self, key: &SigningKey) -> CreatedAuction {
        let (status, body) = self
            .signed(key, Method::POST, "/v1/auctions", Some(auction_input()))
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        serde_json::from_value(body).unwrap()
    }
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn auction_input() -> Value {
    json!({
        "title": "sealForge Zcon7 Pass",
        "description": "10 commemorative passes",
        "network": "testnet",
        "mode": "uniform_price",
        "quantity": 10,
        "reserve_zat": 1_000_000,
        "close_height": 4_000_000,
        "payout_ua": TESTNET_UA,
        "trustees": [
            { "slot": 1, "label": "Organiser" },
            { "slot": 2, "label": "Independent auditor" },
            { "slot": 3, "label": "Community signer" }
        ]
    })
}

/// Stand-in for DKG completion (task 2.4).
async fn mark_ready(db: &PgPool, code: &str) {
    sqlx::query("UPDATE auctions SET status = 'ready', escrow_ua = $1 WHERE code = $2")
        .bind(TESTNET_UA)
        .bind(code)
        .execute(db)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn organiser_registration_requires_invite_and_rejects_replays(db: PgPool) {
    let api = Api::new(db);
    let k = key(1);

    let (status, body) = api
        .signed(
            &k,
            Method::POST,
            "/v1/organisers",
            Some(json!({ "display_name": "Org", "invite_code": "wrong" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let req = || {
        Api::signed_at(
            &k,
            Method::POST,
            "/v1/organisers",
            Some(json!({ "display_name": "Org", "invite_code": INVITE })),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            [9; 16],
        )
    };
    assert_eq!(api.send(req()).await.0, StatusCode::CREATED);
    let (status, body) = api.send(req()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "replayed nonce: {body}");

    let (status, body) = api
        .signed(
            &k,
            Method::POST,
            "/v1/organisers",
            Some(json!({ "display_name": "Org", "invite_code": INVITE })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "already_registered");

    let (status, body) = api.signed(&k, Method::GET, "/v1/organisers/me", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["display_name"], "Org");
}

#[sqlx::test(migrations = "../../migrations")]
async fn rejects_bad_signatures_and_stale_timestamps(db: PgPool) {
    let api = Api::new(db);
    let (status, _) = api
        .send(
            Request::post("/v1/auctions")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unsigned");

    let stale = Api::signed_at(
        &key(1),
        Method::GET,
        "/v1/organisers/me",
        None,
        1_000,
        [1; 16],
    );
    assert_eq!(api.send(stale).await.0, StatusCode::UNAUTHORIZED);

    let mut tampered = Api::signed_at(
        &key(1),
        Method::POST,
        "/v1/auctions",
        Some(json!({ "a": 1 })),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        [2; 16],
    );
    *tampered.body_mut() = Body::from(json!({ "a": 2 }).to_string());
    assert_eq!(api.send(tampered).await.0, StatusCode::UNAUTHORIZED);

    let (status, _) = api
        .signed(&key(5), Method::POST, "/v1/auctions", Some(auction_input()))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "unregistered key");
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_validates_and_hides_drafts(db: PgPool) {
    let api = Api::new(db);
    let k = key(1);
    api.register(&k).await;

    let mut bad = auction_input();
    bad["payout_ua"] = json!("tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd");
    let (status, body) = api
        .signed(&k, Method::POST, "/v1/auctions", Some(bad))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let mut bad = auction_input();
    bad["trustees"] = json!([{ "slot": 1, "label": "A" }, { "slot": 2, "label": "B" }]);
    assert_eq!(
        api.signed(&k, Method::POST, "/v1/auctions", Some(bad))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let created = api.create_auction(&k).await;
    let auction = &created.auction;
    assert!(sealforge_core::memo::is_valid_auction_code(&auction.code));
    assert_eq!(auction.status, AuctionStatus::Draft);
    assert_eq!(auction.trustees.len(), 3);
    assert_eq!(created.trustee_invites.len(), 3);
    assert!(auction.owner.is_some(), "owner sees payout fields");

    let (status, _) = api.public(&format!("/v1/auctions/{}", auction.code)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "drafts are not public");
    let (_, list) = api.public("/v1/auctions").await;
    assert_eq!(list, json!([]));

    let (status, body) = api
        .signed(&k, Method::GET, "/v1/console/auctions", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_open_edit_and_cancel(db: PgPool) {
    let api = Api::new(db.clone());
    let k = key(1);
    api.register(&k).await;
    let code = api.create_auction(&k).await.auction.code;
    let path = format!("/v1/auctions/{code}");

    // Editable while a draft.
    let (status, body) = api
        .signed(&k, Method::PATCH, &path, Some(json!({ "quantity": 12 })))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["quantity"], 12);

    // Can't open before DKG.
    let (status, body) = api
        .signed(&k, Method::POST, &format!("{path}/open"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "invalid_transition");

    mark_ready(&db, &code).await;

    // Chain height unknown → 503. Then a close height that's too soon → 422.
    assert_eq!(
        api.signed(&k, Method::POST, &format!("{path}/open"), None)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    api.state.set_chain_tip(3_999_900);
    assert_eq!(
        api.signed(&k, Method::POST, &format!("{path}/open"), None)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    api.state.set_chain_tip(3_900_000);
    let (status, body) = api
        .signed(&k, Method::POST, &format!("{path}/open"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let view: AuctionView = serde_json::from_value(body).unwrap();
    assert_eq!(view.status, AuctionStatus::Open);
    assert_eq!(view.open_height, Some(3_900_000));

    // Public now, without owner fields.
    let (status, body) = api.public(&path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("owner").is_none());

    // No edits once open.
    let (status, body) = api
        .signed(&k, Method::PATCH, &path, Some(json!({ "quantity": 1 })))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "not_editable");

    // Cancelling an open auction with no bids works.
    let (status, body) = api
        .signed(&k, Method::POST, &format!("{path}/cancel"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "cancelled");

    let log: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'auction.transition'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(log, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn organisers_cannot_touch_each_others_auctions(db: PgPool) {
    let api = Api::new(db);
    let (alice, mallory) = (key(1), key(2));
    api.register(&alice).await;
    api.register(&mallory).await;
    let code = api.create_auction(&alice).await.auction.code;
    let path = format!("/v1/auctions/{code}");

    assert_eq!(
        api.signed(
            &mallory,
            Method::PATCH,
            &path,
            Some(json!({ "quantity": 1 }))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api.signed(&mallory, Method::DELETE, &path, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api.signed(&mallory, Method::GET, &format!("/v1/console{path}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        api.signed(&alice, Method::DELETE, &path, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        api.signed(&alice, Method::GET, &format!("/v1/console{path}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn open_auction_with_bids_cannot_be_cancelled(db: PgPool) {
    let api = Api::new(db.clone());
    let k = key(1);
    api.register(&k).await;
    let code = api.create_auction(&k).await.auction.code;
    mark_ready(&db, &code).await;
    api.state.set_chain_tip(3_900_000);
    assert_eq!(
        api.signed(&k, Method::POST, &format!("/v1/auctions/{code}/open"), None)
            .await
            .0,
        StatusCode::OK
    );

    sqlx::query(
        "INSERT INTO bids (auction_id, txid, output_index, pool, block_height, block_time, amount_zat, status) \
         SELECT id, $2, 0, 'ironwood', 3900001, now(), 5000000, 'valid' FROM auctions WHERE code = $1",
    )
    .bind(&code)
    .bind(&[7u8; 32][..])
    .execute(&db)
    .await
    .unwrap();

    let (status, body) = api
        .signed(
            &k,
            Method::POST,
            &format!("/v1/auctions/{code}/cancel"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "has_bids");
}
