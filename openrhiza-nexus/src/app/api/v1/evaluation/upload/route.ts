import { NextResponse } from "next/server";
import { recordEvaluation } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type EvaluationUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as EvaluationUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    const subjectId = body.subject_id ?? body.driver_id;
    if (!body.node_id || !subjectId) {
      return NextResponse.json(fail("node_id and subject_id (or driver_id) are required."), { status: 400 });
    }

    return NextResponse.json(ok(recordEvaluation(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
