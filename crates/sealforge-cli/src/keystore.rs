//! Local key storage under `$SEALFORGE_HOME` (default `~/.sealforge`).
//! Every file is created owner-only (0600) and never overwritten.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Keystore {
    root: PathBuf,
}

/// A trustee's identity for one auction.
#[derive(Clone, Serialize, Deserialize)]
pub struct TrusteeIdentity {
    pub auction_code: String,
    pub slot: u16,
    pub label: String,
    /// Ed25519 seed (hex): signs API requests and DKG packages.
    pub signing_key: String,
    /// X25519 secret (hex): opens round 2 packages sealed to this trustee.
    pub enc_secret: String,
}

impl TrusteeIdentity {
    pub fn signing_key(&self) -> anyhow::Result<SigningKey> {
        let seed: [u8; 32] = hex::decode(&self.signing_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("bad signing key"))?;
        Ok(SigningKey::from_bytes(&seed))
    }

    pub fn enc_secret(&self) -> anyhow::Result<crypto_box::SecretKey> {
        let bytes: [u8; 32] = hex::decode(&self.enc_secret)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("bad encryption key"))?;
        Ok(crypto_box::SecretKey::from(bytes))
    }
}

impl Keystore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn default_root() -> PathBuf {
        std::env::var_os("SEALFORGE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".sealforge")))
            .unwrap_or_else(|| PathBuf::from(".sealforge"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn organiser_key_path(&self) -> PathBuf {
        self.root.join("organiser.key")
    }

    pub fn create_organiser_key(&self) -> anyhow::Result<SigningKey> {
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        write_private(
            &self.organiser_key_path(),
            hex::encode(key.to_bytes()).as_bytes(),
        )?;
        Ok(key)
    }

    pub fn organiser_key(&self) -> anyhow::Result<SigningKey> {
        let path = self.organiser_key_path();
        let hex_seed = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no organiser key at {} (run `sealforge init`)",
                path.display()
            )
        })?;
        let seed: [u8; 32] = hex::decode(hex_seed.trim())?
            .try_into()
            .map_err(|_| anyhow::anyhow!("organiser key is corrupt"))?;
        Ok(SigningKey::from_bytes(&seed))
    }

    pub fn trustee_dir(&self, auction_code: &str) -> PathBuf {
        self.root.join("trustees").join(auction_code)
    }

    pub fn save_trustee_identity(&self, identity: &TrusteeIdentity) -> anyhow::Result<()> {
        let dir = self.trustee_dir(&identity.auction_code);
        write_private(
            &dir.join("identity.json"),
            serde_json::to_string_pretty(identity)?.as_bytes(),
        )
    }

    pub fn trustee_identity(&self, auction_code: &str) -> anyhow::Result<TrusteeIdentity> {
        let path = self.trustee_dir(auction_code).join("identity.json");
        let json = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "not a trustee of {auction_code} on this machine ({})",
                path.display()
            )
        })?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Read a trustee file, if present.
    pub fn read(&self, auction_code: &str, name: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.trustee_dir(auction_code).join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn write(&self, auction_code: &str, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        write_private(&self.trustee_dir(auction_code).join(name), bytes)
    }
}

/// Create a file readable only by the owner. Never overwrites.
pub fn write_private(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        bail!("{} already exists (refusing to overwrite)", path.display());
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(contents)?;
    Ok(())
}
