import { NextResponse } from "next/server";

import { configuredGeminiModel, generateGeminiText, isGeminiConfigured } from "@/app/google-gemini";
import { fail } from "@/lib/openrhiza-v1";

interface LlmGenerateRequest {
  protocol_version: "v1";
  node_id?: string;
  provider?: "google";
  model?: string;
  prompt?: string;
  system_instruction?: string;
  temperature?: number;
  max_output_tokens?: number;
}

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as LlmGenerateRequest;

    if (body.protocol_version !== "v1") {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.prompt) {
      return NextResponse.json(fail("prompt is required."), { status: 400 });
    }

    if (!isGeminiConfigured()) {
      return NextResponse.json(fail("Google Gemini is not configured on this server.", 503), { status: 503 });
    }

    const result = await generateGeminiText({
      prompt: body.prompt,
      systemInstruction: body.system_instruction,
      model: body.model || configuredGeminiModel(),
      temperature: body.temperature,
      maxOutputTokens: body.max_output_tokens,
    });

    return NextResponse.json({
      success: true,
      data: {
        provider: "google",
        model: result.model,
        text: result.text,
        finish_reason: result.finish_reason,
      },
    });
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
