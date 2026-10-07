-- Local / staging seed data only. Never run against production.
-- Load with: psql "$DATABASE_URL" -f seed/dev.sql

INSERT INTO organisers (id, display_name, pubkey)
VALUES ('00000000-0000-4000-8000-000000000001', 'Demo Organiser',
        decode('3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29', 'hex'))
ON CONFLICT DO NOTHING;

INSERT INTO auctions (id, code, organiser_id, title, description, network, mode, quantity,
                      reserve_zat, close_height, payout_ua, status)
VALUES ('00000000-0000-4000-8000-0000000000a1', 'ZC7PASS001', '00000000-0000-4000-8000-000000000001',
        'sealForge Zcon7 Pass', 'Demo auction: 10 commemorative Zcon7 passes.', 'testnet', 'uniform_price', 10,
        1000000, 0, 'utest1replace-with-a-team-testnet-ua', 'draft')
ON CONFLICT DO NOTHING;

INSERT INTO trustees (auction_id, slot, label, invite_code_hash)
VALUES ('00000000-0000-4000-8000-0000000000a1', 1, 'Organiser', digest('dev-invite-1', 'sha256')),
       ('00000000-0000-4000-8000-0000000000a1', 2, 'Independent auditor', digest('dev-invite-2', 'sha256')),
       ('00000000-0000-4000-8000-0000000000a1', 3, 'Community signer', digest('dev-invite-3', 'sha256'))
ON CONFLICT DO NOTHING;
