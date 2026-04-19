import { NextResponse } from "next/server";
import { uploadGeneratedDriver } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type DriverUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as DriverUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.match_key || !body.payload_text) {
      return NextResponse.json(fail("node_id, match_key, and payload_text are required."), { status: 400 });
    }

    return NextResponse.json(ok(uploadGeneratedDriver(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
