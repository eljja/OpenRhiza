import { NextResponse } from "next/server";
import { uploadSkill } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type SkillUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as SkillUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.skill_id || !body.display_name) {
      return NextResponse.json(fail("node_id, skill_id, and display_name are required."), { status: 400 });
    }

    return NextResponse.json(ok(uploadSkill(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
