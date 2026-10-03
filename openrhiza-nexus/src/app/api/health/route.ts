import { NextResponse } from "next/server";

export async function GET() {
  return NextResponse.json({
    ok: true,
    service: "openrhiza-nexus",
    runtime: "nextjs",
    timestamp: new Date().toISOString(),
  });
}
