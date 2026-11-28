// Serves the hosted testnet site on sealforge.auction.
//
// The app itself still runs on the server behind the Cloudflare Tunnel
// (docs/DEPLOY-VPS.md), reachable at ORIGIN_HOST. This Worker sits on the
// public domain and forwards each request there, so the domain can change
// before mainnet without touching the server.
//
// Privacy: nothing is logged here. Client IPs are not forwarded to the origin
// beyond what Cloudflare itself adds.

const CANONICAL_HOST = "sealforge.auction";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // www and any other alias go to the bare domain.
    if (url.hostname !== CANONICAL_HOST) {
      url.hostname = CANONICAL_HOST;
      return Response.redirect(url.toString(), 301);
    }

    const origin = new URL(request.url);
    origin.hostname = env.ORIGIN_HOST;
    origin.protocol = "https:";
    origin.port = "";

    const headers = new Headers(request.headers);
    // Lets the origin zone tell Worker traffic from direct visits, so the old
    // hostname can redirect people here without looping.
    headers.set("x-sealforge-edge", env.EDGE_KEY);

    const response = await fetch(origin.toString(), {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual",
    });

    // Keep redirects on the public domain.
    const location = response.headers.get("location");
    if (location) {
      const out = new Response(response.body, response);
      out.headers.set(
        "location",
        location.replace(`https://${env.ORIGIN_HOST}`, `https://${CANONICAL_HOST}`),
      );
      return out;
    }
    return response;
  },
};
