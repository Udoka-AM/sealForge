import type { Metadata } from "next";
import localFont from "next/font/local";
import Script from "next/script";
import { HomeChrome } from "@/components/landing/home-chrome";
import { MotionProvider } from "@/components/motion/motion-provider";
import { OfflineBanner } from "@/components/offline-banner";
import { SiteHeader } from "@/components/site-header";
import { WalletProvider } from "@/components/wallet/wallet-provider";
import { CURTAIN_GATE } from "@/lib/curtain";
import "./globals.css";

// Self-hosted from @fontsource: no font requests to Google at build time or at runtime.
// next/font requires literal paths.
const plexSans = localFont({
  src: [
    {
      path: "../../node_modules/@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-400-normal.woff2",
      weight: "400",
      style: "normal",
    },
    {
      path: "../../node_modules/@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-500-normal.woff2",
      weight: "500",
      style: "normal",
    },
    {
      path: "../../node_modules/@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-600-normal.woff2",
      weight: "600",
      style: "normal",
    },
    {
      path: "../../node_modules/@fontsource/ibm-plex-sans/files/ibm-plex-sans-latin-700-normal.woff2",
      weight: "700",
      style: "normal",
    },
  ],
  variable: "--font-plex-sans",
  display: "swap",
});
const plexMono = localFont({
  src: [
    {
      path: "../../node_modules/@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-400-normal.woff2",
      weight: "400",
      style: "normal",
    },
    {
      path: "../../node_modules/@fontsource/ibm-plex-mono/files/ibm-plex-mono-latin-500-normal.woff2",
      weight: "500",
      style: "normal",
    },
  ],
  variable: "--font-plex-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: { default: "sealForge", template: "%s · sealForge" },
  description:
    "Sealed bids. Public price. Private bidders. Auditable sealed-bid auctions on Zcash.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    // The curtain gate adds a class to <html> before hydration.
    <html
      lang="en"
      className={`${plexSans.variable} ${plexMono.variable}`}
      suppressHydrationWarning
    >
      <body className="min-h-dvh antialiased">
        <Script id="sf-curtain-gate" strategy="beforeInteractive">
          {CURTAIN_GATE}
        </Script>
        <MotionProvider>
          <WalletProvider>
            <HomeChrome />
            <div className="overflow-x-clip">
              <SiteHeader />
              <OfflineBanner />
              <main className="mx-auto w-full max-w-[1120px] px-4 py-8 md:px-6 md:py-12">
                {children}
              </main>
            </div>
          </WalletProvider>
        </MotionProvider>
      </body>
    </html>
  );
}
