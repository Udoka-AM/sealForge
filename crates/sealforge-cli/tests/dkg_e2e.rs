//! End to end: organiser + three trustees, each with their own keystore, run
//! the DKG through a real coordinator over HTTP. Then two of the saved key
//! shares produce a consensus-valid spend signature for the escrow.

use std::collections::{BTreeMap, BTreeSet};

use pasta_curves::group::ff::{FromUniformBytes, PrimeField};
use rand::{rngs::OsRng, RngCore};
use sealforge_api::types::{AuctionStatus, ConsoleBatch, DkgState, SpendAuthView};
use sealforge_cli::{
    client::Client,
    keystore::Keystore,
    organiser,
    trustee::{self, DkgProgress, SignProgress},
};
use sealforge_coordinator::{
    routes,
    signing::{self as coordinator_signing, SignerValues},
    AppState, Settings,
};
use sealforge_escrow::{dkg, signing};
use sqlx::PgPool;

const TESTNET_UA: &str = "utest10wa7f6qs6jevt8pl9vezqtrnm9qdkcccdyjtsam4sqgyq4mwvnvxaen2nnsd0yu47qh6a6s0payftzz4h86c6caev4qguam3huf3z7yu";
const DATA_KEY: &str = "e2e-data-key";

async fn start_coordinator(db: PgPool) -> (String, AppState) {
    let state = AppState::new(
        db,
        Settings {
            invite_codes: vec!["org-invite".into()],
            data_key: Some(DATA_KEY.into()),
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = routes::router(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, state)
}

fn random_alpha() -> [u8; 32] {
    let mut wide = [0u8; 64];
    OsRng.fill_bytes(&mut wide);
    pasta_curves::Fq::from_uniform_bytes(&wide).to_repr()
}

#[sqlx::test(migrations = "../../migrations")]
async fn three_trustees_run_the_dkg_through_the_coordinator(db: PgPool) {
    let (url, state) = start_coordinator(db.clone()).await;
    let root = std::env::temp_dir().join(format!("sealforge-e2e-{}", rand::random::<u64>()));
    let org = Keystore::new(root.join("organiser"));
    let trustees: Vec<Keystore> = (1..=3)
        .map(|i| Keystore::new(root.join(format!("trustee-{i}"))))
        .collect();

    // Organiser registers and creates the auction from the example config.
    organiser::init(&org, &url, "org-invite", "Demo Organiser")
        .await
        .unwrap();
    let config = root.join("auction.toml");
    let example = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/auction.toml"
    ))
    .unwrap();
    std::fs::write(&config, example.replace("utest1...", TESTNET_UA)).unwrap();
    let created = organiser::auction_new(&org, &url, &config).await.unwrap();
    let code = created.auction.code.clone();
    assert_eq!(created.trustee_invites.len(), 3);

    // Trustees join. An invite works once.
    for (store, invite) in trustees.iter().zip(&created.trustee_invites) {
        let joined = trustee::join(store, &url, &invite.invite_code)
            .await
            .unwrap();
        assert_eq!(joined.slot, invite.slot);
    }
    let intruder = Keystore::new(root.join("intruder"));
    let reuse = trustee::join(&intruder, &url, &created.trustee_invites[0].invite_code).await;
    assert!(reuse.is_err(), "a used invite must not work twice");

    // Outsiders can't read the DKG, and packages without a valid trustee signature are refused.
    let outsider = Client::new(&url, Some(ed25519_dalek::SigningKey::generate(&mut OsRng)));
    assert!(outsider
        .get::<DkgState>(&format!("/v1/auctions/{code}/dkg"))
        .await
        .is_err());
    let t1_key = trustees[0]
        .trustee_identity(&code)
        .unwrap()
        .signing_key()
        .unwrap();
    let forged = Client::new(&url, Some(t1_key.clone()))
        .post::<DkgState>(
            &format!("/v1/auctions/{code}/dkg/round1"),
            &serde_json::json!({ "payload": "00ff", "signature": hex::encode([0u8; 64]) }),
        )
        .await;
    assert!(
        forged.unwrap_err().to_string().contains("signature"),
        "unsigned package must be rejected"
    );

    // Every trustee keeps stepping until key generation finishes.
    let mut fingerprints = BTreeMap::new();
    let mut escrow = None;
    for _ in 0..10 {
        for (i, store) in trustees.iter().enumerate() {
            match trustee::dkg_step(store, &url, &code).await.unwrap() {
                DkgProgress::Completed {
                    fingerprint,
                    escrow_ua,
                } => {
                    fingerprints.insert(i, fingerprint);
                    escrow = escrow_ua.or(escrow);
                }
                DkgProgress::Ready { escrow_ua } => escrow = Some(escrow_ua),
                _ => {}
            }
        }
        if escrow.is_some() && fingerprints.len() == 3 {
            break;
        }
    }
    let escrow = escrow.expect("DKG finished");
    assert!(escrow.starts_with("utest1"), "{escrow}");
    assert_eq!(
        fingerprints.values().collect::<BTreeSet<_>>().len(),
        1,
        "all trustees derive the same group key"
    );

    // The auction is ready, relay packages are gone, and the UFVK is encrypted at rest.
    let (status, group_key, ufvk): (AuctionStatus, Vec<u8>, String) = sqlx::query_as(
        "SELECT status, escrow_group_key, pgp_sym_decrypt(escrow_ufvk_enc, $2) FROM auctions WHERE code = $1",
    )
    .bind(&code)
    .bind(DATA_KEY)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, AuctionStatus::Ready);
    assert!(ufvk.starts_with("uviewtest1"));
    let leftover: i64 = sqlx::query_scalar("SELECT count(*) FROM dkg_packages")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(leftover, 0);

    // Two trustees' saved shares sign an escrow spend that passes Orchard's check.
    let ak: [u8; 32] = group_key.try_into().unwrap();
    let request = signing::SpendAuthRequest {
        sighash: [42; 32],
        alpha: random_alpha(),
    };
    let (kp1, pubkeys) = trustee::key_share(&trustees[0], &code).unwrap();
    let (kp3, _) = trustee::key_share(&trustees[2], &code).unwrap();
    let (id1, id3) = (dkg::identifier(1).unwrap(), dkg::identifier(3).unwrap());
    let (n1, c1) = signing::commit(&kp1, &mut OsRng);
    let (n3, c3) = signing::commit(&kp3, &mut OsRng);
    let package = signing::signing_package(&request, BTreeMap::from([(id1, c1), (id3, c3)]));
    let shares = BTreeMap::from([
        (id1, signing::sign(&request, &package, &n1, &kp1).unwrap()),
        (id3, signing::sign(&request, &package, &n3, &kp3).unwrap()),
    ]);
    let signature = signing::aggregate(&request, &package, &shares, &pubkeys).unwrap();
    signing::verify_spend_auth(&ak, &request, &signature).unwrap();

    // A two-spend escrow batch goes through `trustee sign` over HTTP.
    let sighash = [7u8; 32];
    let spends = vec![
        SpendAuthView {
            action_index: 0,
            alpha: hex::encode(random_alpha()),
        },
        SpendAuthView {
            action_index: 2,
            alpha: hex::encode(random_alpha()),
        },
    ];
    let batch_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO tx_batches (auction_id, kind, status, sighash, spend_auth, total_out_zat, output_count) \
         SELECT id, 'refund', 'awaiting_signatures', $2, $3, 100000000, 2 FROM auctions WHERE code = $1 RETURNING id",
    )
    .bind(&code)
    .bind(&sighash[..])
    .bind(serde_json::to_value(&spends).unwrap())
    .fetch_one(&db)
    .await
    .unwrap();
    let batch = batch_id.to_string();

    // Trustees 2 and 3 commit first, so they are the signers; trustee 1 is not needed.
    for i in [1, 2] {
        assert_eq!(
            trustee::sign_step(&trustees[i], &url, &code).await.unwrap(),
            vec![SignProgress::Committed {
                batch: batch.clone()
            }]
        );
    }
    let first = trustee::sign_step(&trustees[0], &url, &code).await.unwrap();
    assert!(
        matches!(&first[..], [SignProgress::Committed { .. }]),
        "{first:?}"
    );
    assert_eq!(
        trustee::sign_step(&trustees[0], &url, &code).await.unwrap(),
        vec![SignProgress::NotSelected {
            batch: batch.clone()
        }]
    );
    for i in [1, 2] {
        assert_eq!(
            trustee::sign_step(&trustees[i], &url, &code).await.unwrap(),
            vec![SignProgress::Signed {
                batch: batch.clone()
            }]
        );
        assert!(
            !trustees[i]
                .trustee_dir(&code)
                .join(format!("batch-{batch}.nonces"))
                .exists(),
            "nonces are deleted after use"
        );
    }
    let status: String = sqlx::query_scalar("SELECT status::text FROM tx_batches WHERE id = $1")
        .bind(batch_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "signed");

    // The organiser sees the batch's progress, but no signing material.
    let org_client = Client::new(&url, Some(org.organiser_key().unwrap()));
    let console: Vec<ConsoleBatch> = org_client
        .get(&format!("/v1/console/auctions/{code}/batches"))
        .await
        .unwrap();
    assert_eq!(console.len(), 1);
    assert_eq!(
        (
            console[0].status.as_str(),
            console[0].committed,
            console[0].signed
        ),
        ("signed", 3, 2)
    );
    assert!(
        Client::new(&url, Some(t1_key.clone()))
            .get::<Vec<ConsoleBatch>>(&format!("/v1/console/auctions/{code}/batches"))
            .await
            .is_err(),
        "trustees can't use the organiser view"
    );

    // The coordinator aggregates the posted shares into valid spend signatures.
    let rows: Vec<(i16, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT t.slot, s.commitments_json, s.shares_json FROM signature_shares s \
         JOIN trustees t ON t.id = s.trustee_id WHERE s.batch_id = $1 AND s.shares_json IS NOT NULL",
    )
    .bind(batch_id)
    .fetch_all(&db)
    .await
    .unwrap();
    let signers: BTreeMap<u16, SignerValues> = rows
        .into_iter()
        .map(|(slot, c, s)| {
            (
                slot as u16,
                SignerValues {
                    commitments: serde_json::from_value(c).unwrap(),
                    shares: serde_json::from_value(s).unwrap(),
                },
            )
        })
        .collect();
    assert_eq!(signers.keys().copied().collect::<Vec<_>>(), vec![2, 3]);
    let signatures = coordinator_signing::aggregate(sighash, &spends, &pubkeys, &signers).unwrap();
    assert_eq!(
        signatures.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 2]
    );
    for (spend, (_, sig)) in spends.iter().zip(&signatures) {
        let request = signing::SpendAuthRequest {
            sighash,
            alpha: hex::decode(&spend.alpha).unwrap().try_into().unwrap(),
        };
        signing::verify_spend_auth(&ak, &request, sig).unwrap();
    }

    // A round where a chosen signer disappears is reset, and two others finish it.
    let stalled: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO tx_batches (auction_id, kind, status, sighash, spend_auth, total_out_zat, output_count) \
         SELECT id, 'refund', 'awaiting_signatures', $2, $3, 5000000, 1 FROM auctions WHERE code = $1 RETURNING id",
    )
    .bind(&code)
    .bind(&sighash[..])
    .bind(serde_json::to_value(&spends[..1]).unwrap())
    .fetch_one(&db)
    .await
    .unwrap();
    for i in [0, 1, 0] {
        trustee::sign_step(&trustees[i], &url, &code).await.unwrap();
    }
    // Trustee 2 committed and was chosen, then went quiet for an hour.
    sqlx::query(
        "UPDATE tx_batches SET signers_selected_at = now() - interval '1 hour' WHERE id = $1",
    )
    .bind(stalled)
    .execute(&db)
    .await
    .unwrap();
    let reset = sealforge_coordinator::routes::batches::reset_stalled(
        &state,
        std::time::Duration::from_secs(1800),
    )
    .await
    .unwrap();
    assert_eq!(reset, 1);
    for i in [2, 1, 2, 1] {
        trustee::sign_step(&trustees[i], &url, &code).await.unwrap();
    }
    let (status, slots): (String, Vec<i16>) =
        sqlx::query_as("SELECT status::text, signer_slots FROM tx_batches WHERE id = $1")
            .bind(stalled)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((status.as_str(), slots), ("signed", vec![3, 2]));

    // And the organiser can open it.
    state.set_chain_tip(3_900_000);
    let opened = organiser::auction_open(&org, &url, &code).await.unwrap();
    assert_eq!(opened.status, AuctionStatus::Open);
    assert_eq!(opened.escrow_ua.as_deref(), Some(escrow.as_str()));

    std::fs::remove_dir_all(&root).ok();
}
