import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "OpenRhiza.com",
  description: "Registry and coordination surface for OpenRhiza nodes, drivers, software, and LLM metadata.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="h-full antialiased">
      <body className="min-h-full flex flex-col">{children}</body>
    </html>
  );
}
