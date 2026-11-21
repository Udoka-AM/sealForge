# Shared tamper cases (vendored)

`tamper/` is copied unchanged from frost-zec at commit `c417ced`
(<https://github.com/OpenPGP-Wallet/frost-zec/tree/c417ced/vectors/tamper>),
MIT License, Copyright (c) 2026 Khadim. The format is frost-zec spec § 8.2;
the refusal classes are in `tamper/classes.md`.

sealForge is a listed maintainer on frost-zec#7. To update:

1. Copy `vectors/tamper/` from the new frost-zec commit over this directory.
2. Update the commit above and in `crates/sealforge-verify/src/frost_zec/`
   headers if the checks changed upstream.
3. Run `cargo test -p sealforge-verify --test tamper_vectors`. Every case must
   pass; report any format friction in frost-zec#7, not here.

Embargoed cases (sent privately to the frost-zec#7 maintainers before
publication) must not be committed here or run in public CI until frost-zec
publishes them. Run them locally against a private copy.
