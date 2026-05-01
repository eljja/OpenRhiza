import { NextResponse } from "next/server";
import { uploadPolicy } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type PolicyUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as PolicyUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.policy_id || !body.scope) {
      return NextResponse.json(fail("node_id, policy_id, and scope are required."), { status: 400 });
    }

    return NextResponse.json(ok(uploadPolicy(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
