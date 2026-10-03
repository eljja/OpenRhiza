import { NextResponse } from "next/server";
import { uploadSoftwarePackage } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type SoftwareUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as SoftwareUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.package_id || !body.display_name) {
      return NextResponse.json(fail("node_id, package_id, and display_name are required."), { status: 400 });
    }

    return NextResponse.json(ok(uploadSoftwarePackage(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
