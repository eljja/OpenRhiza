import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type EvaluationUploadRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as EvaluationUploadRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.driver_id) {
      return NextResponse.json(fail("node_id and driver_id are required."), { status: 400 });
    }

    return NextResponse.json(
      ok({
        evaluation_id: `eval_${body.node_id}_${body.driver_id}`,
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
