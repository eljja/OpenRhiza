import { NextResponse } from "next/server";
import { addDriverComment, getDriver } from "@/app/registry-data";
import { fail, isV1Protocol, ok, type DriverCommentRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as DriverCommentRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.driver_id || !body.comment) {
      return NextResponse.json(fail("node_id, driver_id, and comment are required."), { status: 400 });
    }

    if (!getDriver(body.driver_id)) {
      return NextResponse.json(fail(`Driver not found: ${body.driver_id}`), { status: 404 });
    }

    return NextResponse.json(ok(addDriverComment(body)));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
