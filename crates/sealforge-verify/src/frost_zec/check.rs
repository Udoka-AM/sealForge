// Vendored from frost-zec (https://github.com/OpenPGP-Wallet/frost-zec) at commit
// c417ced, MIT License, Copyright (c) 2026 Khadim. See ../../LICENSE-frost-zec.
// Kept as close to upstream as possible so the shared signer checks (frost-zec
// spec § 6) and tamper cases (§ 8.2, frost-zec#7) stay comparable. Local
// changes are marked "sealForge:".

//! What the co-signer checks before it signs, on its own.
//!
//! It does not trust the amounts and addresses written in the PCZT: for every
//! action it recomputes the note commitments, the nullifier, the randomized
//! key and the value commitment from those fields (orchard's PCZT verifiers),
//! so a lying coordinator cannot show one payment and get a signature for
//! another. Then it sorts the outputs into payments and change, and works out
//! the fee.

use std::collections::BTreeSet;
use std::error::Error;

use ff::{Field, PrimeField};
use orchard::keys::{FullViewingKey, PreparedIncomingViewingKey, Scope};
use orchard::note::ExtractedNoteCommitment;
use orchard::{Anchor, Note};
use pasta_curves::pallas;
use pczt::roles::low_level_signer::Signer;
use pczt::Pczt;
use zcash_address::unified::{self, Container, Encoding};
use zcash_protocol::consensus::NetworkType;
use zcash_protocol::constants::V6_TX_VERSION;
use zcash_protocol::PoolType;

use super::keys;
use super::pczt_io::TxError;

/// Above this fee the co-signer warns (ZIP 317 fees are normally 0.0001 to
/// 0.0003 ZEC for a few actions).
const HIGH_FEE: u64 = 100_000;

/// ZIP 317's marginal fee per logical action, in zatoshis. ZIP 2009 (Proposed)
/// lowers it to 1,000 from block 3,590,000 on Mainnet and 4,465,026 on testnet.
/// The co-signer sees no chain, so it accepts any fee up to the old rate for
/// the actions a payment needs: both rates pass around that height.
const MAX_MARGINAL_FEE: u64 = 5_000;

/// ZIP 317's grace actions: a bundle is padded to at least two actions.
const GRACE_ACTIONS: usize = 2;

/// A recent block of each network, its height and time (Unix seconds), and a
/// block spacing faster than the network ever keeps up for long. From these and
/// this computer's clock, the co-signer bounds the current height from above.
///
/// The target spacing is 25 seconds since NU7 (ZIP 218). Mainnet keeps to it
/// (25.0 s on average from 2026-11-09 to 2026-11-12), so 20 s leaves room. Testnet
/// runs faster (about 18 s on 2026-11-12), so it is counted at 10 s.
const CHECKPOINTS: [(NetworkType, u32, u64, u64); 2] = [
    // 2026-11-10 15:39:24 UTC
    (NetworkType::Main, 3_558_933, 1_794_325_164, 20),
    // 2026-11-12 16:49:09 UTC
    (NetworkType::Test, 4_612_900, 1_794_502_149, 10),
];

/// The target spacing since NU7, to say roughly when the expiry falls. Shown on
/// Mainnet only: testnet does not keep to it.
const TARGET_BLOCK_SECS: u64 = 25;

/// How far beyond that estimate an expiry may be. Wallets set it minutes ahead
/// (zcash_primitives: 120 blocks since NU7, 50 minutes).
const EXPIRY_MARGIN_BLOCKS: u64 = 1_000;

/// The total supply of ZEC in zatoshis: no honest amount can be larger.
const MAX_MONEY: u64 = 21_000_000 * 100_000_000;

/// What the co-signer shows before asking for approval, and the same facts for
/// a program. `lines` and `warnings` are shown to the person who approves; the
/// other fields carry the same facts for an automated signer, such as a
/// spending policy, so it never has to parse text. More facts may be added:
/// read the fields, never build a `Report`.
#[derive(Debug)]
#[non_exhaustive]
pub struct Report {
    /// The transaction, line by line: spends, payments and their memos,
    /// change, fee, lock time.
    pub lines: Vec<String>,
    /// What the user must know before approving: the v6 pre-authorization
    /// notice, a high fee.
    pub warnings: Vec<String>,
    /// Every output that leaves the group with value, in action order.
    pub payments: Vec<Payment>,
    /// The value of the group's notes this transaction spends.
    pub spent_zat: u64,
    /// The value that comes back to the group: change, and sends to its own
    /// addresses.
    pub change_zat: u64,
    /// The fee the transaction pays, in zatoshis.
    pub fee_zat: u64,
    /// The height after which the transaction can no longer be mined.
    pub expiry_height: u32,
}

/// One payment leaving the group.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Payment {
    /// The receiver that is paid, encoded on its own as a unified address. It
    /// is not the address the PCZT names for display: that one is checked to
    /// hold this receiver, but its other receivers are not checked against
    /// anything, so a program that compares addresses must use this one.
    pub address: String,
    /// The value paid to that receiver, in zatoshis.
    pub zat: u64,
}

/// An amount in zatoshis, written in ZEC with eight decimals.
pub fn zec(zatoshis: u64) -> String {
    amount(zatoshis, NetworkType::Main)
}

/// The unit amounts are written in: testnet coins are TAZ, not ZEC.
pub fn unit(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Main => "ZEC",
        NetworkType::Test | NetworkType::Regtest => "TAZ",
    }
}

/// An amount in zatoshis, with eight decimals, in the unit of `network`.
pub fn amount(zatoshis: u64, network: NetworkType) -> String {
    format!(
        "{}.{:08} {}",
        zatoshis / 100_000_000,
        zatoshis % 100_000_000,
        unit(network)
    )
}

/// Adds an amount read from the PCZT to a running total. The amounts come
/// from the coordinator, so they are bounded instead of trusted not to overflow.
fn add(total: &mut u64, value: u64) -> Result<(), TxError> {
    *total = total
        .checked_add(value)
        .filter(|sum| *sum <= MAX_MONEY)
        .ok_or_else(|| TxError::Refused("amounts larger than the total ZEC supply".into()))?;
    Ok(())
}

/// Checks on one spend that need no key.
///
/// - ZIP 374: "Signers MUST reject PCZTs that contain `dummy_sk` values". The
///   IO Finalizer signs the dummy spends with that key and then clears it, so a
///   PCZT that still holds one skipped a step, and its signer would be handed a
///   spending key.
/// - When the PCZT gives a witness for a real note, it must lead to the
///   bundle's anchor. This only catches an inconsistent PCZT: the anchor itself
///   comes from the coordinator, and a v6 signature does not commit to it (see
///   `pre_authorization`).
///
/// A zero-value spend has no place in the tree to check: the circuit skips the
/// Merkle path for it.
fn check_spend_structure(
    index: usize,
    spend: &orchard::pczt::Spend,
    anchor: &Anchor,
    anchor_deferred: bool,
) -> Result<(), TxError> {
    if spend.dummy_sk().is_some() {
        return Err(TxError::Refused(format!(
            "action {index}: the PCZT still holds a dummy spending key (ZIP 374: a signer must refuse it)"
        )));
    }
    let value = spend.value().map(|v| v.inner()).unwrap_or_default();
    let (false, Some(witness), true) = (anchor_deferred, spend.witness(), value > 0) else {
        return Ok(());
    };
    let incomplete = || TxError::Refused(format!("action {index}: incomplete spent note"));
    let note = Note::from_parts(
        spend.recipient().ok_or_else(incomplete)?,
        spend.value().ok_or_else(incomplete)?,
        spend.rho().ok_or_else(incomplete)?,
        spend.rseed().ok_or_else(incomplete)?,
        *spend.note_version(),
    )
    .into_option()
    .ok_or_else(incomplete)?;
    if witness.root(ExtractedNoteCommitment::from(note.commitment())) != *anchor {
        return Err(TxError::Refused(format!(
            "action {index}: the note's witness does not lead to the transaction's anchor"
        )));
    }
    Ok(())
}

/// A spend's randomizer `alpha` is what hides the signing key on chain: the
/// spend shows `rk = ak + [alpha]G`. A zero `alpha` would publish the group's
/// `ak` itself as `rk`, the same in every such spend, and an `alpha` used for
/// two spends gives them the same `rk`: either way anyone could link them.
/// orchard's builder draws it at random, so only a dishonest or broken builder
/// gets here. Reuse across transactions would need the co-signer to remember
/// every `alpha` it signed; it does not.
fn check_randomizer(
    index: usize,
    alpha: &pallas::Scalar,
    seen: &mut BTreeSet<[u8; 32]>,
) -> Result<(), TxError> {
    if bool::from(alpha.is_zero()) {
        return Err(TxError::Refused(format!(
            "action {index}: the spend's randomizer is zero, which would show the group's key on chain"
        )));
    }
    if !seen.insert(alpha.to_repr()) {
        return Err(TxError::Refused(format!(
            "action {index}: two spends share a randomizer, which would link them on chain"
        )));
    }
    Ok(())
}

/// The address a payment is shown with.
///
/// A PCZT may name the address the user was given (`user_address`, ZIP 374):
/// zcash_client_backend writes the payment's address there, usually a unified
/// address with several receivers. That is the string the user can compare
/// with their invoice, so it is shown, once checked: it must be a unified
/// address on this network, and its Orchard receiver (the one Ironwood notes
/// go to) must be the one paid. Any other value is refused, not ignored: a
/// coordinator that names one address and pays another is not honest. Without
/// it, the paid receiver alone is shown, as a unified address.
fn payment_address(
    index: usize,
    named: Option<&str>,
    recipient: &orchard::Address,
    network: NetworkType,
) -> Result<String, TxError> {
    let Some(named) = named else {
        return keys::encode_address(recipient, network)
            .map_err(|e| TxError::Refused(e.to_string()));
    };
    let not_paid = || {
        TxError::Refused(format!(
            "action {index}: the address the transaction names is not the one it pays"
        ))
    };
    let (named_network, _, address) = unified::Address::decode(named).map_err(|_| not_paid())?;
    let paid = recipient.to_raw_address_bytes();
    let pays_it = address
        .items()
        .iter()
        .any(|item| matches!(item, unified::Receiver::Orchard(bytes) if *bytes == paid));
    if named_network != network || !pays_it {
        return Err(not_paid());
    }
    // Shown as encoded again from what was parsed, never as the PCZT's bytes.
    Ok(address.encode(&network))
}

/// The warning every v6 signature needs. A v6 sighash leaves the anchors out
/// (ZIP 374 lets them be set after signing), so signing does not prove that the
/// spent notes exist on chain: it authorizes exactly these payments for as long
/// as the transaction can be mined, whenever those notes exist. The expiry
/// height is what bounds that. A transaction that never expires is refused, and
/// so is one that expires far beyond the current height as this computer's clock
/// bounds it: a coordinator could keep such a signature and broadcast it much
/// later, for example after the user, told it failed, approved the same payment
/// again. `now` is the clock, in Unix seconds.
fn pre_authorization(
    expiry_height: u32,
    network: NetworkType,
    now: u64,
) -> Result<String, TxError> {
    if expiry_height == 0 {
        return Err(TxError::Refused(
            "the transaction never expires: a pre-authorized v6 spend would stay valid forever"
                .into(),
        ));
    }
    let mut when = String::new();
    let checkpoint = CHECKPOINTS.iter().find(|(net, ..)| *net == network);
    if let Some(&(_, height, time, fastest)) = checkpoint {
        let elapsed = now.saturating_sub(time);
        let highest = u64::from(height) + elapsed / fastest;
        if u64::from(expiry_height) > highest + EXPIRY_MARGIN_BLOCKS {
            return Err(TxError::Refused(format!(
                "the transaction stays valid until block {expiry_height}, far beyond the current \
                 height (at most about {highest} by this computer's clock): a signature must not \
                 stay usable that long. If this computer's clock is wrong, set it right"
            )));
        }
        if network == NetworkType::Main {
            let expected = u64::from(height) + elapsed / TARGET_BLOCK_SECS;
            when = match u64::from(expiry_height).checked_sub(expected) {
                Some(blocks) => {
                    format!(", about {} from now", duration(blocks * TARGET_BLOCK_SECS))
                }
                None => ", a height already passed by this computer's clock".into(),
            };
        }
    }
    Ok(format!(
        "v6 pre-authorization: this signature does not prove the spent notes exist on chain; \
         it authorizes these exact payments if they do, until block {expiry_height}{when}"
    ))
}

/// A rough duration, for the expiry warning.
fn duration(secs: u64) -> String {
    match secs {
        0..7_200 => format!("{} min", secs.div_ceil(60)),
        7_200..172_800 => format!("{} h", secs / 3_600),
        _ => format!("{} days", secs / 86_400),
    }
}

/// This computer's clock, in Unix seconds (0 if it is set before 1970).
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Verifies and describes a PCZT that spends from the group `fvk`, with this
/// computer's clock.
pub fn analyze(
    pczt: &Pczt,
    fvk: &FullViewingKey,
    network: NetworkType,
) -> Result<Report, Box<dyn Error>> {
    analyze_at(pczt, fvk, network, now())
}

/// [`analyze`] with the clock given, in Unix seconds: what bounds the expiry
/// height. For tests and shared tamper cases, which fix the time they are
/// checked at (spec § 8.2).
pub fn analyze_at(
    pczt: &Pczt,
    fvk: &FullViewingKey,
    network: NetworkType,
    now: u64,
) -> Result<Report, Box<dyn Error>> {
    // Every part of the transaction is covered by the signature. A part this
    // co-signer does not decode could carry value away without being shown:
    // value spent from the group's Ironwood notes and paid out in, say, an
    // Orchard bundle would only look like a larger fee here.
    for (pool, name) in [
        (PoolType::Transparent, "transparent"),
        (PoolType::SAPLING, "Sapling"),
        (PoolType::ORCHARD, "Orchard"),
    ] {
        if pczt.has_data_in_pool(pool) {
            return Err(format!(
                "the transaction has {name} parts, which the co-signer does not decode: refused"
            )
            .into());
        }
    }

    let mut lines = Vec::new();
    let mut warnings = Vec::new();
    let mut spent = 0u64;
    let mut paid = 0u64;
    let mut change = 0u64;
    let mut value_balance = None;
    let mut randomizers = BTreeSet::new();
    let mut payments = Vec::new();
    // What bounds the fee: how many actions the transaction has, and how many
    // it needs to move its value.
    let mut actions = 0usize;
    let mut valued_spends = 0usize;
    let mut valued_outputs = 0usize;
    // Zero-value notes back to the group itself: the wallet's change output
    // when the change is exactly zero (see the action count below).
    let mut own_zero_outputs = 0usize;

    if *pczt.global().tx_version() == V6_TX_VERSION {
        warnings.push(pre_authorization(
            *pczt.global().expiry_height(),
            network,
            now,
        )?);
    }
    if let Some(lock_time) = pczt.global().fallback_lock_time().filter(|t| *t != 0) {
        lines.push(format!("not valid before lock time {lock_time}"));
    }

    let external_ovk = fvk.to_ovk(Scope::External);
    let internal_ovk = fvk.to_ovk(Scope::Internal);
    let external_ivk = PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::External));
    let internal_ivk = PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::Internal));

    Signer::new(pczt.clone()).sign_ironwood_with(|_, bundle, _| -> Result<(), TxError> {
        let refuse = |what: &str, error: &dyn std::fmt::Debug| {
            TxError::Refused(format!("{what}: {error:?}"))
        };
        bundle
            .verify_cross_address_restriction()
            .map_err(|e| refuse("cross-address check failed", &e))?;
        let anchor = *bundle.anchor();
        let anchor_deferred = *bundle.anchor_deferred();
        let bundle_version = *bundle.bundle_version();
        value_balance = Some(
            i64::try_from(*bundle.value_sum())
                .map_err(|e| refuse("bundle value balance out of range", &e))?,
        );
        for (index, action) in bundle.actions().iter().enumerate() {
            let spend = action.spend();
            let output = action.output();
            check_spend_structure(index, spend, &anchor, anchor_deferred)?;
            // The value commitment only binds this action's net value. The
            // note checks below are what bind the claimed values and recipient.
            action
                .verify_cv_net()
                .map_err(|e| refuse(&format!("action {index}: value commitment"), &e))?;
            let spend_value = spend.value().map(|v| v.inner()).unwrap_or_default();
            let output_value = output.value().map(|v| v.inner()).unwrap_or_default();

            if spend_value == 0 {
                // A dummy spend: it pads the transaction, belongs to a throwaway
                // key and was already signed when the PCZT was built. It moves no
                // value; an unsigned zero-value spend is not expected.
                if spend.spend_auth_sig().is_none() {
                    return Err(TxError::Refused(format!(
                        "action {index}: unsigned zero-value spend"
                    )));
                }
            } else {
                spend
                    .verify_nullifier(Some(fvk))
                    .map_err(|e| refuse(&format!("action {index}: spent note"), &e))?;
                spend
                    .verify_rk(Some(fvk))
                    .map_err(|e| refuse(&format!("action {index}: randomized key"), &e))?;
                let alpha = spend.alpha().as_ref().ok_or_else(|| {
                    TxError::Refused(format!("action {index}: the spend has no randomizer"))
                })?;
                check_randomizer(index, alpha, &mut randomizers)?;
            }
            output
                .verify_note_commitment(spend)
                .map_err(|e| refuse(&format!("action {index}: output note"), &e))?;
            add(&mut spent, spend_value)?;
            actions += 1;
            if spend_value > 0 {
                valued_spends += 1;
                lines.push(format!(
                    "spend {} from this group",
                    amount(spend_value, network)
                ));
            }
            if output_value == 0 {
                // A zero-value output is signed like any other. The builder's
                // padding cannot be read by anyone; one the coordinator added
                // and encrypted to the group's outgoing key can, and it would
                // land on chain with a recipient and memo nobody was shown. One
                // encrypted without that key cannot be told from padding: the
                // action count below leaves no room for more than the padding.
                let recovered = action
                    .recover_output_with_ovk(&external_ovk, bundle_version)
                    .and_then(|found| match found {
                        Some(found) => Ok(Some(found)),
                        None => action.recover_output_with_ovk(&internal_ovk, bundle_version),
                    })
                    .map_err(|e| refuse(&format!("action {index}: note encryption"), &e))?;
                if let Some((_, address, memo)) = recovered {
                    if fvk.scope_for_address(&address).is_none() {
                        return Err(TxError::Refused(format!(
                            "action {index}: a zero-value note to another address, with a memo \
                             the user would not be shown"
                        )));
                    }
                    own_zero_outputs += 1;
                    lines.push(format!(
                        "send 0 {} to this group's own address",
                        unit(network)
                    ));
                    lines.push(format!("    {}", describe_memo(&memo)));
                } else if let Some(recipient) = output
                    .recipient()
                    .filter(|r| fvk.scope_for_address(r) == Some(Scope::Internal))
                {
                    // Zero change: the wallet encrypts change with no outgoing
                    // key, so it is read back with the group's internal
                    // incoming key, as valued change is below.
                    let (note, address, memo) = action
                        .decrypt_output_with_ivk(&internal_ivk, bundle_version)
                        .map_err(|e| refuse(&format!("action {index}: note encryption"), &e))?
                        .ok_or_else(|| {
                            TxError::Refused(format!(
                                "action {index}: a zero-value note to this group that it \
                                 cannot decrypt"
                            ))
                        })?;
                    if address != recipient || note.value().inner() != 0 {
                        return Err(TxError::Refused(format!(
                            "action {index}: the encrypted note differs from the one described"
                        )));
                    }
                    own_zero_outputs += 1;
                    lines.push(format!("change 0 {} back to this group", unit(network)));
                    lines.push(format!("    {}", describe_memo(&memo)));
                }
            } else {
                valued_outputs += 1;
                let recipient = output.recipient().ok_or_else(|| {
                    TxError::Refused(format!("action {index}: output without recipient"))
                })?;
                // The note ciphertext is signed too, but nothing above checks
                // it. Decrypting it proves that it holds this very note, so its
                // recipient can find it, and gives the memo actually sent.
                let scope = fvk.scope_for_address(&recipient);
                let decrypted = match scope {
                    // The group's own output (change, or a payment to itself):
                    // decrypted with its incoming viewing key, as its wallet
                    // will when scanning. Wallets often encrypt change with no
                    // outgoing key, so this is the check that always applies.
                    Some(Scope::Internal) => {
                        action.decrypt_output_with_ivk(&internal_ivk, bundle_version)
                    }
                    Some(Scope::External) => {
                        action.decrypt_output_with_ivk(&external_ivk, bundle_version)
                    }
                    // A payment: read back with the group's outgoing viewing key.
                    None => action
                        .recover_output_with_ovk(&external_ovk, bundle_version)
                        .and_then(|found| match found {
                            Some(found) => Ok(Some(found)),
                            None => action.recover_output_with_ovk(&internal_ovk, bundle_version),
                        }),
                }
                .map_err(|e| refuse(&format!("action {index}: note encryption"), &e))?;
                let (note, address, memo) = decrypted.ok_or_else(|| {
                    TxError::Refused(match scope {
                        Some(_) => format!(
                            "action {index}: the group cannot decrypt its own output, so its \
                             wallet would never find these funds"
                        ),
                        None => format!(
                            "action {index}: the payment cannot be read back with this group's \
                             outgoing viewing key, so the co-signer cannot tell what it sends"
                        ),
                    })
                })?;
                if address != recipient || note.value().inner() != output_value {
                    return Err(TxError::Refused(format!(
                        "action {index}: the encrypted note differs from the one described"
                    )));
                }
                match scope {
                    Some(Scope::Internal) => {
                        add(&mut change, output_value)?;
                        lines.push(format!(
                            "change {} back to this group",
                            amount(output_value, network)
                        ));
                    }
                    Some(Scope::External) => {
                        add(&mut change, output_value)?;
                        lines.push(format!(
                            "send {} to this group's own address",
                            amount(output_value, network)
                        ));
                    }
                    None => {
                        add(&mut paid, output_value)?;
                        let address = payment_address(
                            index,
                            output.user_address().as_deref(),
                            &recipient,
                            network,
                        )?;
                        lines.push(format!(
                            "PAY {} to {address}",
                            amount(output_value, network)
                        ));
                        lines.push(format!("    {}", describe_memo(&memo)));
                        payments.push(Payment {
                            address: keys::encode_address(&recipient, network)
                                .map_err(|e| TxError::Refused(e.to_string()))?,
                            zat: output_value,
                        });
                    }
                }
            }
        }
        Ok(())
    })?;

    let mut outputs = paid;
    add(&mut outputs, change)?;
    let fee = spent
        .checked_sub(outputs)
        .ok_or("the outputs are worth more than the spent notes")?;
    // The signed balance of the bundle is what actually leaves it. With no
    // other part in the transaction, all of it is the fee.
    if value_balance != Some(fee as i64) {
        return Err(format!(
            "the bundle's signed value balance ({value_balance:?}) is not the fee shown ({fee})"
        )
        .into());
    }
    lines.push(format!("fee {}", amount(fee, network)));
    // ZIP 317 charges per action, so a coordinator could pad the transaction
    // with zero-value actions and make a raised fee look like the honest fee
    // of a bigger transaction. The builder pads a bundle to two actions and no
    // further: more than that, beyond the notes spent and paid, is refused,
    // and so is a fee above ZIP 317's rate for the actions that remain. This
    // does not catch a coordinator that spends more notes than needed or
    // splits the change: the co-signer does not know which notes a payment
    // should spend.
    //
    // One exception: when a payment leaves exactly zero change, the standard
    // wallet (zcash_client_backend) still adds a zero-value change note to the
    // sender, so that the number of payees and their sum don't give away the
    // inputs. With one payee that note fits in the padding; with two (a payout
    // and its protocol fee) it is a third action. One such note, readable and
    // sent to this group's own address, is allowed; it costs at most one
    // action's fee. Any more is padding.
    let needed = valued_spends
        .max(valued_outputs + own_zero_outputs.min(1))
        .max(GRACE_ACTIONS);
    if actions > needed {
        return Err(format!(
            "the transaction has {actions} actions where its payments need {needed}: the \
             extra ones move no value"
        )
        .into());
    }
    let max_fee = MAX_MARGINAL_FEE * needed as u64;
    if fee > max_fee {
        return Err(format!(
            "the fee ({}) is more than ZIP 317 asks for {needed} actions ({})",
            amount(fee, network),
            amount(max_fee, network)
        )
        .into());
    }
    if fee > HIGH_FEE {
        warnings.push(format!("high fee: {}", amount(fee, network)));
    }
    if spent == 0 {
        return Err("the transaction spends nothing from this group".into());
    }
    Ok(Report {
        lines,
        warnings,
        payments,
        spent_zat: spent,
        change_zat: change,
        fee_zat: fee,
        expiry_height: *pczt.global().expiry_height(),
    })
}

/// Text from a transaction or from another device, made safe to show:
/// control characters, and characters that reorder or hide text, are escaped
/// (`\n`, `\u{202e}`), so the text cannot draw fake lines or rewrite the ones
/// around it. Everything else stays as it is, accents and quotes included.
pub fn printable(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() || reorders_or_hides(c) {
            shown.extend(c.escape_default());
        } else {
            shown.push(c);
        }
    }
    shown
}

/// Invisible format characters: soft hyphen, direction marks, embeddings,
/// overrides and isolates, zero-width characters, the byte order mark,
/// interlinear annotations and tags.
fn reorders_or_hides(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E007F}'
    )
}

/// The memo as ZIP 302 defines it, made safe to show with [`printable`].
pub fn describe_memo(memo: &[u8; 512]) -> String {
    match memo[0] {
        0xF6 if memo[1..].iter().all(|b| *b == 0) => "no memo".into(),
        0x00..=0xF4 => {
            let end = memo.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
            match std::str::from_utf8(&memo[..end]) {
                Ok(text) => format!("memo: \"{}\"", printable(text)),
                Err(_) => "memo: text that is not valid UTF-8".into(),
            }
        }
        kind => format!("memo: binary data (type 0x{kind:02x})"),
    }
}

// sealForge: upstream unit tests removed (they use upstream fixtures); the
// shared tamper cases in tests/tamper_vectors.rs cover these checks.
