import { NextResponse } from "next/server";
import { downloadDriverArtifact } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type DriverDownloadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as DriverDownloadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id) {
      return NextResponse.json(fail("node_id is required."), { status: 400 });
    }

    if (!body.driver_id && !body.match_key) {
      return NextResponse.json(fail("driver_id or match_key is required."), { status: 400 });
    }

    return NextResponse.json(ok(downloadDriverArtifact(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
