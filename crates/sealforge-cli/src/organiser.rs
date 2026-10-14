//! Organiser commands (task 2.6).

use std::path::Path;

use anyhow::Context;
use sealforge_api::types::{
    AuctionView, CreateAuction, CreatedAuction, OrganiserView, RegisterOrganiser,
};

use crate::{client::Client, keystore::Keystore};

/// `sealforge init`: create an organiser key and register it.
pub async fn init(
    store: &Keystore,
    coordinator: &str,
    invite_code: &str,
    name: &str,
) -> anyhow::Result<OrganiserView> {
    let key = store.create_organiser_key()?;
    let client = Client::new(coordinator, Some(key));
    client
        .post(
            "/v1/organisers",
            &RegisterOrganiser {
                display_name: name.to_owned(),
                invite_code: invite_code.to_owned(),
            },
        )
        .await
        .context("registering with the coordinator (the key is saved; retry with the same invite)")
}

fn client(store: &Keystore, coordinator: &str) -> anyhow::Result<Client> {
    Ok(Client::new(coordinator, Some(store.organiser_key()?)))
}

/// `sealforge auction new <config.toml>`
pub async fn auction_new(
    store: &Keystore,
    coordinator: &str,
    config: &Path,
) -> anyhow::Result<CreatedAuction> {
    let text =
        std::fs::read_to_string(config).with_context(|| format!("reading {}", config.display()))?;
    let input: CreateAuction =
        toml::from_str(&text).with_context(|| format!("parsing {}", config.display()))?;
    client(store, coordinator)?
        .post("/v1/auctions", &input)
        .await
}

pub async fn auction_list(store: &Keystore, coordinator: &str) -> anyhow::Result<Vec<AuctionView>> {
    client(store, coordinator)?
        .get("/v1/console/auctions")
        .await
}

pub async fn auction_show(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<AuctionView> {
    client(store, coordinator)?
        .get(&format!("/v1/console/auctions/{code}"))
        .await
}

pub async fn auction_open(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<AuctionView> {
    client(store, coordinator)?
        .post_empty(&format!("/v1/auctions/{code}/open"))
        .await
}

pub async fn auction_cancel(
    store: &Keystore,
    coordinator: &str,
    code: &str,
) -> anyhow::Result<AuctionView> {
    client(store, coordinator)?
        .post_empty(&format!("/v1/auctions/{code}/cancel"))
        .await
}
