import { NextResponse } from "next/server";
import { downloadSkillArtifact } from "@/app/registry-data";
import { fail, isV1Protocol, type SkillDownloadRequest, ok } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as SkillDownloadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id) {
      return NextResponse.json(fail("node_id is required."), { status: 400 });
    }

    if (!body.skill_id) {
      return NextResponse.json(fail("skill_id is required."), { status: 400 });
    }

    return NextResponse.json(ok(downloadSkillArtifact(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
