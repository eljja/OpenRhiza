import { NextResponse } from "next/server";

import { isGeminiConfigured, listGeminiModels } from "@/app/google-gemini";
import { fail } from "@/lib/openrhiza-v1";

export async function GET() {
  try {
    if (!isGeminiConfigured()) {
      return NextResponse.json(fail("Google Gemini is not configured on this server.", 503), { status: 503 });
    }

    const payload = await listGeminiModels();
    const models = (payload.models ?? [])
      .filter((model) => model.supportedActions?.includes("generateContent"))
      .map((model) => ({
        name: model.name ?? "",
        display_name: model.displayName ?? model.name ?? "",
        description: model.description ?? "",
      }));

    return NextResponse.json({
      success: true,
      data: {
        provider: "google",
        models,
      },
    });
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
