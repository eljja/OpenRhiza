const GEMINI_BASE_URL = "https://generativelanguage.googleapis.com/v1beta";

export function isGeminiConfigured() {
  return Boolean(process.env.GOOGLE_GEMINI_API_KEY);
}

export function configuredGeminiModel() {
  return process.env.OPENRHIZA_GEMINI_MODEL || "gemini-2.5-flash";
}

export async function listGeminiModels() {
  const apiKey = process.env.GOOGLE_GEMINI_API_KEY;
  if (!apiKey) {
    throw new Error("GOOGLE_GEMINI_API_KEY is not configured.");
  }

  const response = await fetch(`${GEMINI_BASE_URL}/models?key=${encodeURIComponent(apiKey)}`, {
    method: "GET",
    cache: "no-store",
  });

  if (!response.ok) {
    throw new Error(`Gemini models request failed with status ${response.status}.`);
  }

  return (await response.json()) as {
    models?: Array<{
      name?: string;
      displayName?: string;
      description?: string;
      supportedActions?: string[];
    }>;
  };
}

export async function generateGeminiText(input: {
  prompt: string;
  systemInstruction?: string;
  model?: string;
  temperature?: number;
  maxOutputTokens?: number;
}) {
  const apiKey = process.env.GOOGLE_GEMINI_API_KEY;
  if (!apiKey) {
    throw new Error("GOOGLE_GEMINI_API_KEY is not configured.");
  }

  const model = input.model || configuredGeminiModel();
  const body: Record<string, unknown> = {
    contents: [
      {
        parts: [{ text: input.prompt }],
      },
    ],
    generationConfig: {
      temperature: input.temperature ?? 0.2,
      maxOutputTokens: input.maxOutputTokens ?? 2048,
    },
  };

  if (input.systemInstruction) {
    body.systemInstruction = {
      parts: [{ text: input.systemInstruction }],
    };
  }

  const response = await fetch(
    `${GEMINI_BASE_URL}/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      cache: "no-store",
      body: JSON.stringify(body),
    },
  );

  const payload = (await response.json()) as {
    candidates?: Array<{
      content?: {
        parts?: Array<{ text?: string }>;
      };
      finishReason?: string;
    }>;
    promptFeedback?: unknown;
    error?: {
      message?: string;
    };
  };

  if (!response.ok) {
    throw new Error(payload.error?.message || `Gemini generateContent failed with status ${response.status}.`);
  }

  const text = payload.candidates
    ?.flatMap((candidate) => candidate.content?.parts ?? [])
    .map((part) => part.text ?? "")
    .join("")
    .trim();

  return {
    model,
    text: text || "",
    finish_reason: payload.candidates?.[0]?.finishReason ?? null,
    raw: payload,
  };
}
