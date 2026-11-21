//! Local key storage under `$SEALFORGE_HOME` (default `~/.sealforge`).
//! Every file is created owner-only (0600). Key files are never overwritten;
//! the one record that grows (the refund ledger) is replaced atomically.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Keystore {
    root: PathBuf,
}

/// A trustee's long-term identity, the same across auctions (stream 4). Its
/// public key is published somewhere the trustee controls (`published_at`),
/// and in each auction it signs a certificate for the per-auction key.
#[derive(Clone, Serialize, Deserialize)]
pub struct LongTermIdentity {
    /// Ed25519 seed (hex).
    pub signing_key: String,
    /// Where the trustee publishes the public key: a URL or handle.
    #[serde(default)]
    pub published_at: Option<String>,
}

impl LongTermIdentity {
    pub fn signing_key(&self) -> anyhow::Result<SigningKey> {
        let seed: [u8; 32] = hex::decode(&self.signing_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("bad long-term identity key"))?;
        Ok(SigningKey::from_bytes(&seed))
    }

    pub fn public_key_hex(&self) -> anyhow::Result<String> {
        Ok(hex::encode(self.signing_key()?.verifying_key().to_bytes()))
    }
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

    fn long_term_identity_path(&self) -> PathBuf {
        self.root.join("trustee-identity.json")
    }

    /// This trustee's long-term identity, if it made one.
    pub fn long_term_identity(&self) -> anyhow::Result<Option<LongTermIdentity>> {
        let path = self.long_term_identity_path();
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes)
                    .with_context(|| format!("reading {}", path.display()))?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Save the long-term identity: created once, then only the place it is
    /// published may change (replaced atomically). The key never changes.
    pub fn save_long_term_identity(&self, identity: &LongTermIdentity) -> anyhow::Result<()> {
        let path = self.long_term_identity_path();
        let bytes = serde_json::to_vec_pretty(identity)?;
        if !path.exists() {
            return write_private(&path, &bytes);
        }
        let tmp = self.root.join("trustee-identity.json.tmp");
        std::fs::remove_file(&tmp).ok();
        write_private(&tmp, &bytes)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
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

    /// Replace a trustee record atomically: the new contents are written and
    /// synced to a temporary file, which is then renamed over the old one, so
    /// a crash leaves either the old record or the new one, never a torn file.
    /// For records that grow (the refund ledger), never for keys.
    pub fn replace(&self, auction_code: &str, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let dir = self.trustee_dir(auction_code);
        let path = dir.join(name);
        let tmp = dir.join(format!("{name}.tmp"));
        // A leftover from a crash mid-write was never renamed into place.
        match std::fs::remove_file(&tmp) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("removing {}", tmp.display())),
        }
        write_private(&tmp, bytes)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        // Make the rename itself durable.
        #[cfg(unix)]
        std::fs::File::open(&dir)?.sync_all()?;
        Ok(())
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
    // Durable before the caller acts on it (e.g. the signed record must survive
    // a crash before the shares it covers are sent).
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Keystore;

    #[test]
    fn replace_overwrites_a_record_that_write_refuses_to() {
        let root = std::env::temp_dir().join(format!(
            "sf-keystore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Keystore::new(&root);
        store.write("CODE", "ledger.json", b"[1]").unwrap();
        assert!(store.write("CODE", "ledger.json", b"[1,2]").is_err());
        store.replace("CODE", "ledger.json", b"[1,2]").unwrap();
        store.replace("CODE", "ledger.json", b"[1,2,3]").unwrap();
        assert_eq!(
            store.read("CODE", "ledger.json").unwrap().unwrap(),
            b"[1,2,3]"
        );
        // A crash leftover doesn't block the next replace.
        std::fs::write(store.trustee_dir("CODE").join("ledger.json.tmp"), b"torn").unwrap();
        store.replace("CODE", "ledger.json", b"[4]").unwrap();
        assert_eq!(store.read("CODE", "ledger.json").unwrap().unwrap(), b"[4]");
        assert!(!store.trustee_dir("CODE").join("ledger.json.tmp").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
