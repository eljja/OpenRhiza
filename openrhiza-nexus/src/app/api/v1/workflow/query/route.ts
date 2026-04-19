import { NextResponse } from "next/server";
import { queryWorkflows } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type WorkflowQueryRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as WorkflowQueryRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    return NextResponse.json(ok(queryWorkflows(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
