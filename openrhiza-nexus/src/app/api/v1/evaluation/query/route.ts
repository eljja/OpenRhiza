import { NextResponse } from "next/server";
import { queryEvaluations } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type EvaluationQueryRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as EvaluationQueryRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    return NextResponse.json(ok(queryEvaluations(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
