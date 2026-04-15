import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type NodeRegisterRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as NodeRegisterRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.public_key) {
      return NextResponse.json(fail("node_id and public_key are required."), { status: 400 });
    }

    return NextResponse.json(
      ok({
        node: {
          node_id: body.node_id,
          trust_tier: (body.identity_type === "tpm_key" ? "tpm" : "software") as "tpm" | "software",
        },
        server: {
          protocol_version: "v1",
          min_heartbeat_interval_ms: 30000,
        },
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
