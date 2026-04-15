import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type SoftwareQueryRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as SoftwareQueryRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    return NextResponse.json(
      ok({
        packages: [
          {
            package_id: "pkg_terminal_tools_v1",
            display_name: "Terminal Starter Tools",
            summary: "Basic CLI-first package set for networked OpenRhiza systems.",
          },
        ],
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
