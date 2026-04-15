import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type NodeHeartbeatRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as NodeHeartbeatRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.hardware_fingerprint) {
      return NextResponse.json(fail("node_id and hardware_fingerprint are required."), { status: 400 });
    }

    return NextResponse.json(
      ok({
        server_time: new Date().toISOString(),
        next_actions: [],
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
