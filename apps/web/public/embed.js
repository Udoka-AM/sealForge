/*! sealForge auction card (task 4.4). MIT OR Apache-2.0.
 *
 * <script src="https://<sealforge-site>/embed.js" data-auction="AUCTIONCODE" async></script>
 *
 * Renders a small card (in a shadow root, so your styles and ours don't clash)
 * right where the script tag is. It reads only public auction data and sets no
 * cookies. Bidding opens on the sealForge site in a new tab.
 */
(() => {
  const script = document.currentScript;
  if (!script) return;
  const code = script.getAttribute("data-auction");
  if (!code || !/^[0-9A-HJKMNP-TV-Z]{10}$/.test(code)) return;
  const origin = new URL(script.src).origin;

  const host = document.createElement("div");
  host.setAttribute("data-sealforge-card", code);
  script.after(host);
  const root = host.attachShadow({ mode: "open" });

  const zec = (zat) => {
    const whole = Math.floor(zat / 1e8);
    const frac = String(zat % 1e8)
      .padStart(8, "0")
      .replace(/0+$/, "");
    return frac ? `${whole}.${frac} ZEC` : `${whole} ZEC`;
  };
  const eta = (blocks) => {
    const mins = Math.round((blocks * 75) / 60);
    if (mins < 60) return `~${mins} min`;
    return `~${Math.floor(mins / 60)} h${mins % 60 ? ` ${mins % 60} min` : ""}`;
  };
  const el = (tag, attrs, ...children) => {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) n.setAttribute(k, v);
    for (const c of children) n.append(c);
    return n;
  };

  const style = el(
    "style",
    {},
    `:host{all:initial}
.card{--bg:#13161b;--text:#eceae4;--muted:#9aa0a9;--border:#262b33;--primary:#f4b728;--ink:#0b0d10;
font:15px/1.45 "IBM Plex Sans",system-ui,sans-serif;background:var(--bg);color:var(--text);
border:1px solid var(--border);border-radius:14px;padding:18px;max-width:360px;box-sizing:border-box}
@media (prefers-color-scheme:light){.card{--bg:#fff;--text:#16181c;--muted:#5d636c;--border:#e3e1da;--primary:#b9850a;--ink:#fff}}
.brand{font-size:12px;color:var(--muted);margin:0 0 6px}.brand b{color:var(--primary);font-weight:600}
h3{font-size:18px;line-height:1.3;margin:0 0 12px;font-weight:600}
dl{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin:0 0 14px}
dt{font-size:12px;color:var(--muted)}dd{margin:0;font-variant-numeric:tabular-nums}
a.btn{display:block;text-align:center;text-decoration:none;border-radius:10px;padding:11px 14px;font-weight:500}
a.primary{background:var(--primary);color:var(--ink)}a.secondary{border:1px solid var(--border);color:var(--text)}
a.btn:focus-visible{outline:2px solid var(--primary);outline-offset:2px}
p.note{font-size:12px;color:var(--muted);margin:10px 0 0}`,
  );
  const card = el("div", { class: "card", role: "region", "aria-label": "sealForge auction" });
  card.append(el("p", { class: "brand" }, "Sealed-bid auction on ", el("b", {}, "sealForge")));
  root.append(style, card);

  fetch(`${origin}/api/embed/${code}`)
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((a) => {
      card.append(el("h3", {}, a.title));
      const closes = a.accepting_bids
        ? a.blocks_left === null
          ? `block ${a.close_height}`
          : `in ${eta(a.blocks_left)}`
        : "closed";
      card.append(
        el(
          "dl",
          {},
          el("div", {}, el("dt", {}, "Lots"), el("dd", {}, String(a.quantity))),
          el("div", {}, el("dt", {}, "Reserve"), el("dd", {}, zec(a.reserve_zat))),
          el("div", {}, el("dt", {}, "Closes"), el("dd", {}, closes)),
        ),
      );
      const published = ["published", "refunding", "settled"].includes(a.status);
      const link = a.accepting_bids
        ? el(
            "a",
            { class: "btn primary", href: a.url, target: "_blank", rel: "noopener" },
            "Bid with Zcash",
          )
        : published
          ? el(
              "a",
              { class: "btn secondary", href: a.audit_url, target: "_blank", rel: "noopener" },
              "See the audited result",
            )
          : el(
              "a",
              { class: "btn secondary", href: a.url, target: "_blank", rel: "noopener" },
              "View auction",
            );
      card.append(link);
      card.append(
        el(
          "p",
          { class: "note" },
          a.sealed_reveal
            ? "Bids stay sealed, even from the trustees, until bidders reveal them after the close."
            : "Bids are shielded Zcash payments. The price is public after close; bidders aren't.",
        ),
      );
    })
    .catch(() => {
      card.append(el("p", { class: "note" }, "This auction can't be loaded right now."));
    });
})();
