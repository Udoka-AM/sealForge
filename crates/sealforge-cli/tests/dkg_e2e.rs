//! End to end: organiser + three trustees, each with their own keystore, run
//! the DKG through a real coordinator over HTTP. Then two of the saved key
//! shares produce a consensus-valid spend signature for the escrow.

use std::collections::{BTreeMap, BTreeSet};

use pasta_curves::group::ff::{FromUniformBytes, PrimeField};
use rand::{rngs::OsRng, RngCore};
use sealforge_api::types::{
    AuctionStatus, AuctionView, BatchView, ConsoleBatch, DkgState, PostInputs, SpendAuthView,
};
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

/// The protocol tests below drive signing with fake batches (a made-up sighash,
/// no transaction). Real trustees refuse those; these tests turn the
/// transaction checks off to exercise the nonce and share handling alone. The
/// checks themselves are tested on real PCZTs in sealforge-verify.
async fn sign_fake(store: &Keystore, url: &str, code: &str) -> anyhow::Result<Vec<SignProgress>> {
    trustee::sign_step_with(
        store,
        url,
        code,
        trustee::SignPolicy {
            skip_transaction_checks_for_tests: true,
            ..Default::default()
        },
    )
    .await
}

/// Post a chosen signer's signed statement of its round 2 inputs, as the CLI
/// does before signing, without signing yet. Lets a test put both statements
/// in place first, so the signing order below is deterministic.
async fn state_inputs(store: &Keystore, url: &str, code: &str, batch: &str) {
    let identity = store.trustee_identity(code).unwrap();
    let key = identity.signing_key().unwrap();
    let client = Client::new(url, Some(key.clone()));
    let views: Vec<BatchView> = client
        .get(&format!("/v1/auctions/{code}/batches"))
        .await
        .unwrap();
    let view = views.into_iter().find(|b| b.id == batch).unwrap();
    let alphas: Vec<[u8; 32]> = view
        .spends
        .iter()
        .map(|s| hex::decode(&s.alpha).unwrap().try_into().unwrap())
        .collect();
    let commitments: BTreeMap<u16, Vec<Vec<u8>>> = view
        .signer_commitments
        .iter()
        .map(|(slot, values)| {
            (
                *slot,
                values.iter().map(|c| hex::decode(c).unwrap()).collect(),
            )
        })
        .collect();
    let sighash: [u8; 32] = hex::decode(&view.sighash).unwrap().try_into().unwrap();
    let digest = sealforge_api::auth::round2_inputs_digest(&sighash, &alphas, &commitments);
    let signature =
        sealforge_api::auth::sign_round2_inputs(&key, code, batch, identity.slot, &digest);
    let _: BatchView = client
        .post(
            &format!("/v1/batches/{batch}/inputs"),
            &PostInputs {
                digest: hex::encode(digest),
                signature: hex::encode(signature),
            },
        )
        .await
        .unwrap();
}

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

    // Trustees join, all at once: the last join must still start the DKG.
    let invites = &created.trustee_invites;
    let (a, b, c) = tokio::join!(
        trustee::join(&trustees[0], &url, &invites[0].invite_code),
        trustee::join(&trustees[1], &url, &invites[1].invite_code),
        trustee::join(&trustees[2], &url, &invites[2].invite_code),
    );
    for (joined, invite) in [a, b, c].into_iter().zip(invites) {
        assert_eq!(joined.unwrap().slot, invite.slot);
    }
    // An invite works once.
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
        // All three at once, as on real machines: this is where DKG writes race.
        let (a, b, c) = tokio::join!(
            trustee::dkg_step(&trustees[0], &url, &code),
            trustee::dkg_step(&trustees[1], &url, &code),
            trustee::dkg_step(&trustees[2], &url, &code),
        );
        for (i, progress) in [a, b, c].into_iter().enumerate() {
            match progress.unwrap() {
                DkgProgress::Completed {
                    fingerprint,
                    escrow_ua,
                } => {
                    fingerprints.insert(i, fingerprint);
                    escrow = escrow_ua.or(escrow);
                }
                DkgProgress::Ready { escrow_ua } | DkgProgress::Verified { escrow_ua } => {
                    escrow = Some(escrow_ua)
                }
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

    // A coordinator showing an address the trustees' key doesn't control is
    // caught by every trustee, and the auction can't open.
    sqlx::query(
        "UPDATE trustees SET escrow_verified_at = NULL, escrow_attestation = NULL \
         WHERE auction_id = (SELECT id FROM auctions WHERE code = $1)",
    )
    .bind(&code)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("UPDATE auctions SET escrow_ua = $2 WHERE code = $1")
        .bind(&code)
        .bind(TESTNET_UA)
        .execute(&db)
        .await
        .unwrap();
    for t in &trustees {
        let err = trustee::dkg_step(t, &url, &code).await.unwrap_err();
        assert!(err.to_string().contains("ESCROW ADDRESS MISMATCH"), "{err}");
    }
    let refused = organiser::auction_open(&org, &url, &code)
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("verify the escrow"),
        "{refused}"
    );

    // With the real address, each trustee rebuilds it from its own group key
    // and signs an attestation anyone can check with the trustee's public key.
    sqlx::query("UPDATE auctions SET escrow_ua = $2 WHERE code = $1")
        .bind(&code)
        .bind(&escrow)
        .execute(&db)
        .await
        .unwrap();
    for t in &trustees {
        assert_eq!(
            trustee::dkg_step(t, &url, &code).await.unwrap(),
            DkgProgress::Verified {
                escrow_ua: escrow.clone()
            }
        );
        assert_eq!(
            trustee::dkg_step(t, &url, &code).await.unwrap(),
            DkgProgress::Ready {
                escrow_ua: escrow.clone()
            }
        );
    }
    let attestations: Vec<(i16, Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT slot, pubkey, escrow_attestation FROM trustees \
         WHERE auction_id = (SELECT id FROM auctions WHERE code = $1) ORDER BY slot",
    )
    .bind(&code)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(attestations.len(), 3);
    for (slot, pubkey, signature) in attestations {
        assert!(sealforge_api::auth::verify_escrow_attestation(
            &pubkey.try_into().unwrap(),
            &signature.try_into().unwrap(),
            &code,
            slot as u16,
            &escrow,
        ));
    }

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

    // One trustee alone can't sign (PRD AC10): FROST refuses a one-signer package,
    // and one share can't be aggregated into a signature.
    let (n1b, c1b) = signing::commit(&kp1, &mut OsRng);
    let lone = signing::signing_package(&request, BTreeMap::from([(id1, c1b)]));
    assert!(signing::sign(&request, &lone, &n1b, &kp1).is_err());
    let one_share = BTreeMap::from([(id1, shares[&id1])]);
    assert!(signing::aggregate(&request, &package, &one_share, &pubkeys).is_err());

    // Trustees refuse to co-sign while bidders can't see the auction page
    // (F15): a 404 is not "nothing to check".
    let err = trustee::sign_step(&trustees[0], &url, &code)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Refusing to co-sign"), "{err}");

    // The organiser opens it; from here the public page shows the attested address.
    state.set_chain_tip(3_900_000);
    let opened = organiser::auction_open(&org, &url, &code).await.unwrap();
    assert_eq!(opened.status, AuctionStatus::Open);
    assert_eq!(opened.escrow_ua.as_deref(), Some(escrow.as_str()));

    // A keystore with no record of the attested address refuses too, unless the
    // trustee itself opts in for an auction from before attestations.
    let attested = trustees[0].trustee_dir(&code).join("escrow-attested.txt");
    let record = std::fs::read(&attested).unwrap();
    std::fs::remove_file(&attested).unwrap();
    let err = trustee::sign_step(&trustees[0], &url, &code)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("no record of the escrow address"),
        "{err}"
    );
    let allowed = trustee::SignPolicy {
        allow_unattested_escrow: true,
        skip_transaction_checks_for_tests: true,
        ..Default::default()
    };
    assert!(
        trustee::sign_step_with(&trustees[0], &url, &code, allowed.clone())
            .await
            .is_ok()
    );
    std::fs::write(&attested, record).unwrap();

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

    // A real trustee refuses to take part: there is no transaction to check (D1).
    let err = trustee::sign_step(&trustees[1], &url, &code)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no transaction to check"), "{err}");

    // Trustees 2 and 3 commit first, so they are the signers; trustee 1 is not needed.
    for i in [1, 2] {
        assert_eq!(
            sign_fake(&trustees[i], &url, &code).await.unwrap(),
            vec![SignProgress::Committed {
                batch: batch.clone()
            }]
        );
    }
    let first = sign_fake(&trustees[0], &url, &code).await.unwrap();
    assert!(
        matches!(&first[..], [SignProgress::Committed { .. }]),
        "{first:?}"
    );
    assert_eq!(
        sign_fake(&trustees[0], &url, &code).await.unwrap(),
        vec![SignProgress::NotSelected {
            batch: batch.clone()
        }]
    );
    // Before any share leaves, each signer states the round 2 inputs it signs
    // over; a signer whose co-signer hasn't stated yet waits.
    assert_eq!(
        sign_fake(&trustees[1], &url, &code).await.unwrap(),
        vec![SignProgress::WaitingForCoSignerCheck {
            batch: batch.clone()
        }]
    );
    state_inputs(&trustees[2], &url, &code, &batch).await;
    // Both signers post their shares at the same moment; the batch must still become `signed`.
    let (s1, s2) = tokio::join!(
        sign_fake(&trustees[1], &url, &code),
        sign_fake(&trustees[2], &url, &code),
    );
    for (i, progress) in [(1, s1), (2, s2)] {
        assert_eq!(
            progress.unwrap(),
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
        sign_fake(&trustees[i], &url, &code).await.unwrap();
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
    for i in [2, 1] {
        sign_fake(&trustees[i], &url, &code).await.unwrap();
    }
    for i in [2, 1] {
        state_inputs(&trustees[i], &url, &code, &stalled.to_string()).await;
    }
    for i in [2, 1] {
        sign_fake(&trustees[i], &url, &code).await.unwrap();
    }
    let (status, slots): (String, Vec<i16>) =
        sqlx::query_as("SELECT status::text, signer_slots FROM tx_batches WHERE id = $1")
            .bind(stalled)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((status.as_str(), slots), ("signed", vec![3, 2]));

    // A coordinator that "loses" a share and then asks for one over a different
    // sighash must not get a second share over the same nonces (two shares
    // over one nonce pair, on different messages, reveal the signing share).
    let replayed: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO tx_batches (auction_id, kind, status, sighash, spend_auth, total_out_zat, output_count) \
         SELECT id, 'refund', 'awaiting_signatures', $2, $3, 5000000, 1 FROM auctions WHERE code = $1 RETURNING id",
    )
    .bind(&code)
    .bind(&sighash[..])
    .bind(serde_json::to_value(&spends[..1]).unwrap())
    .fetch_one(&db)
    .await
    .unwrap();
    let replayed_id = replayed.to_string();
    for i in [0, 1] {
        let progress = sign_fake(&trustees[i], &url, &code).await.unwrap();
        assert!(
            progress.contains(&SignProgress::Committed {
                batch: replayed_id.clone()
            }),
            "{progress:?}"
        );
    }
    state_inputs(&trustees[1], &url, &code, &replayed_id).await;
    let progress = sign_fake(&trustees[0], &url, &code).await.unwrap();
    assert!(
        progress.contains(&SignProgress::Signed {
            batch: replayed_id.clone()
        }),
        "{progress:?}"
    );
    let share_of_slot_1 = |db: PgPool| async move {
        sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT s.shares_json FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
             WHERE s.batch_id = $1 AND t.slot = 1",
        )
        .bind(replayed)
        .fetch_one(&db)
        .await
        .unwrap()
    };
    let first_share = share_of_slot_1(db.clone()).await.expect("share stored");
    let other_sighash: [u8; 32] = random_alpha();
    sqlx::query("UPDATE signature_shares SET shares_json = NULL WHERE batch_id = $1")
        .bind(replayed)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE tx_batches SET sighash = $2 WHERE id = $1")
        .bind(replayed)
        .bind(&other_sighash[..])
        .execute(&db)
        .await
        .unwrap();
    let progress = sign_fake(&trustees[0], &url, &code).await.unwrap();
    assert!(
        progress.contains(&SignProgress::InputsChanged {
            batch: replayed_id.clone()
        }),
        "a trustee never signs new inputs with nonces it already used: {progress:?}"
    );
    assert_eq!(share_of_slot_1(db.clone()).await, None, "nothing was sent");
    // Back to the original inputs, the trustee resends exactly what it signed.
    sqlx::query("UPDATE tx_batches SET sighash = $2 WHERE id = $1")
        .bind(replayed)
        .bind(&sighash[..])
        .execute(&db)
        .await
        .unwrap();
    let progress = sign_fake(&trustees[0], &url, &code).await.unwrap();
    assert!(
        progress.contains(&SignProgress::Signed {
            batch: replayed_id.clone()
        }),
        "{progress:?}"
    );
    assert_eq!(share_of_slot_1(db.clone()).await, Some(first_share));
    assert!(!trustees[0]
        .trustee_dir(&code)
        .join(format!("batch-{replayed_id}.nonces"))
        .exists());

    // A coordinator that shows the two signers different commitment sets is
    // caught before either releases a share (stream 2c).
    let equivocated: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO tx_batches (auction_id, kind, status, sighash, spend_auth, total_out_zat, output_count) \
         SELECT id, 'refund', 'awaiting_signatures', $2, $3, 5000000, 1 FROM auctions WHERE code = $1 RETURNING id",
    )
    .bind(&code)
    .bind(&sighash[..])
    .bind(serde_json::to_value(&spends[..1]).unwrap())
    .fetch_one(&db)
    .await
    .unwrap();
    let equivocated_id = equivocated.to_string();
    for i in [0, 1] {
        sign_fake(&trustees[i], &url, &code).await.unwrap();
    }
    state_inputs(&trustees[1], &url, &code, &equivocated_id).await;
    // Slot 2 stated its inputs over the real set; slot 1 is now shown slot 1's
    // own commitment in place of slot 2's.
    // Rows by slot: `$2` is the slot whose row changes, `$3` the one copied from.
    sqlx::query(
        "UPDATE signature_shares SET commitments_json = ( \
           SELECT s.commitments_json FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
           WHERE s.batch_id = $1 AND t.slot = $3) \
         WHERE batch_id = $1 AND trustee_id = ( \
           SELECT s.trustee_id FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
           WHERE s.batch_id = $1 AND t.slot = $2)",
    )
    .bind(equivocated)
    .bind(2i16)
    .bind(1i16)
    .execute(&db)
    .await
    .unwrap();
    let err = sign_fake(&trustees[0], &url, &code).await.unwrap_err();
    assert!(
        err.to_string().contains("(equivocation)")
            && err.to_string().contains("different round 2 inputs"),
        "{err}"
    );
    // A forged statement, slot 2's signature over slot 1's digest, doesn't verify.
    sqlx::query(
        "UPDATE signature_shares SET inputs_json = jsonb_set(inputs_json, '{digest}', ( \
           SELECT s.inputs_json->'digest' FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
           WHERE s.batch_id = $1 AND t.slot = $3)) \
         WHERE batch_id = $1 AND trustee_id = ( \
           SELECT s.trustee_id FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
           WHERE s.batch_id = $1 AND t.slot = $2)",
    )
    .bind(equivocated)
    .bind(2i16)
    .bind(1i16)
    .execute(&db)
    .await
    .unwrap();
    let err = sign_fake(&trustees[0], &url, &code).await.unwrap_err();
    assert!(
        err.to_string().contains("(equivocation)")
            && err
                .to_string()
                .contains("doesn't verify under its public key"),
        "{err}"
    );
    let slot_1_share: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT s.shares_json FROM signature_shares s JOIN trustees t ON t.id = s.trustee_id \
         WHERE s.batch_id = $1 AND t.slot = 1",
    )
    .bind(equivocated)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(slot_1_share, None, "no share left slot 1");
    sqlx::query("UPDATE tx_batches SET status = 'failed' WHERE id = $1")
        .bind(equivocated)
        .execute(&db)
        .await
        .unwrap();

    // Bidders can check the escrow themselves (F10): the public page lists each
    // trustee's key and attestation, and they verify for the address shown.
    let public: AuctionView = Client::new(&url, None)
        .get(&format!("/v1/auctions/{code}"))
        .await
        .unwrap();
    let keys: Vec<(u16, String)> = trustees
        .iter()
        .enumerate()
        .map(|(i, t)| (i as u16 + 1, trustee::my_key(t, &code).unwrap()))
        .collect();
    let check = sealforge_auditor::check_escrow_view(&public, &keys);
    assert!(check.verified(), "{check:?}");
    assert_eq!((check.signed, check.total), (3, 3));
    for t in &trustees {
        assert_eq!(
            trustee::check_public(t, &url, &code).await.unwrap(),
            trustee::PublicCheck::Matches {
                escrow_ua: escrow.clone(),
                signed: 3,
                total: 3,
                keys: keys
                    .iter()
                    .map(|(slot, k)| (*slot, hex::decode(k).unwrap().try_into().unwrap()))
                    .collect(),
            }
        );
    }

    // Backups (stream 3): each trustee kept the escrow's viewing key when it
    // attested, and its backup string checks out against its own keystore only.
    let mut backups = Vec::new();
    for t in &trustees {
        backups.push(trustee::backup(t, &url, &code).await.unwrap());
    }
    for (i, t) in trustees.iter().enumerate() {
        assert!(t.trustee_dir(&code).join("escrow-view.json").exists());
        let check = trustee::check_backup(t, &code, &backups[i]).unwrap();
        assert_eq!(
            (check.slot, check.escrow_ua.as_str()),
            (i as u16 + 1, escrow.as_str())
        );
        let other = &backups[(i + 1) % 3];
        assert!(
            trustee::check_backup(t, &code, other).is_err(),
            "another trustee's backup"
        );
    }
    // Two backup strings alone, no coordinator and no keystore, rebuild the
    // escrow address and sign a spend that verifies under ak.
    {
        use sealforge_escrow::backup::{recover_group, Backup};
        let pair: Vec<Backup> = [&backups[0], &backups[2]]
            .into_iter()
            .map(|s| Backup::decode(s).unwrap())
            .collect();
        assert_eq!(
            sealforge_escrow::address::escrow_address(
                &pair[0].full_viewing_key(),
                &sealforge_escrow::address::EncodingParams(pair[0].network),
            )
            .unwrap(),
            escrow
        );
        let (restored, group) = recover_group(&pair).unwrap();
        let request = signing::SpendAuthRequest {
            sighash: random_alpha(),
            alpha: random_alpha(),
        };
        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for (id, kp) in &restored {
            let (n, c) = signing::commit(kp, &mut OsRng);
            nonces.insert(*id, n);
            commitments.insert(*id, c);
        }
        let package = signing::signing_package(&request, commitments);
        let shares = nonces
            .iter()
            .map(|(id, n)| {
                (
                    *id,
                    signing::sign(&request, &package, n, &restored[id]).unwrap(),
                )
            })
            .collect();
        let sig = signing::aggregate(&request, &package, &shares, &group).unwrap();
        signing::verify_spend_auth(&pair[0].ak(), &request, &sig).unwrap();
    }

    // A coordinator that shows bidders another address fails every check, and
    // no trustee will co-sign money out of the escrow while it does.
    sqlx::query("UPDATE auctions SET escrow_ua = $2 WHERE code = $1")
        .bind(&code)
        .bind(TESTNET_UA)
        .execute(&db)
        .await
        .unwrap();
    let public: AuctionView = Client::new(&url, None)
        .get(&format!("/v1/auctions/{code}"))
        .await
        .unwrap();
    assert!(!sealforge_auditor::check_escrow_view(&public, &[]).verified());
    for t in &trustees {
        let err = trustee::check_public(t, &url, &code).await.unwrap_err();
        assert!(err.to_string().contains("PUBLIC PAGE MISMATCH"), "{err}");
        let err = trustee::sign_step(t, &url, &code).await.unwrap_err();
        assert!(err.to_string().contains("PUBLIC PAGE MISMATCH"), "{err}");
    }
    sqlx::query("UPDATE auctions SET escrow_ua = $2 WHERE code = $1")
        .bind(&code)
        .bind(&escrow)
        .execute(&db)
        .await
        .unwrap();

    // A coordinator that swaps in a trustee key it controls can sign for that
    // slot, so the attestations alone still verify. The replaced trustee
    // catches it, and so does any bidder who pinned that trustee's real key.
    let fake = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
    let fake_sig = sealforge_api::auth::sign_escrow_attestation(&fake, &code, 2, &escrow);
    let (real_pubkey, real_sig): (Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT pubkey, escrow_attestation FROM trustees \
         WHERE slot = 2 AND auction_id = (SELECT id FROM auctions WHERE code = $1)",
    )
    .bind(&code)
    .fetch_one(&db)
    .await
    .unwrap();
    let set_slot_2 = |pubkey: Vec<u8>, sig: Vec<u8>| {
        let (db, code) = (db.clone(), code.clone());
        async move {
            sqlx::query(
                "UPDATE trustees SET pubkey = $2, escrow_attestation = $3 \
                 WHERE slot = 2 AND auction_id = (SELECT id FROM auctions WHERE code = $1)",
            )
            .bind(&code)
            .bind(pubkey)
            .bind(sig)
            .execute(&db)
            .await
            .unwrap();
        }
    };
    set_slot_2(fake.verifying_key().to_bytes().to_vec(), fake_sig.to_vec()).await;
    let public: AuctionView = Client::new(&url, None)
        .get(&format!("/v1/auctions/{code}"))
        .await
        .unwrap();
    assert!(sealforge_auditor::check_escrow_view(&public, &[]).verified());
    assert!(!sealforge_auditor::check_escrow_view(&public, &keys).verified());
    let err = trustee::check_public(&trustees[1], &url, &code)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("PUBLIC PAGE MISMATCH"), "{err}");
    set_slot_2(real_pubkey, real_sig).await;
    assert!(matches!(
        trustee::check_public(&trustees[1], &url, &code)
            .await
            .unwrap(),
        trustee::PublicCheck::Matches { .. }
    ));

    std::fs::remove_dir_all(&root).ok();
}
