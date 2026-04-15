import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type LlmQueryRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as LlmQueryRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    return NextResponse.json(
      ok({
        models: [
          {
            model_id: "llm_remote_general_v1",
            display_name: "OpenRhiza Remote General Model",
            mode: "remote_api",
            summary: "General-purpose remote inference endpoint for early OpenRhiza nodes.",
          },
        ],
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
