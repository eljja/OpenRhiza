import { NextResponse } from "next/server";
import { archiveUploadedDriver } from "@/app/registry-data";

export async function POST(req: Request) {
  try {
    const data = await req.json();
    const archived = archiveUploadedDriver(data);

    return NextResponse.json({
      success: true,
      message: archived.message,
      id: archived.id,
    });
  } catch (err) {
    return NextResponse.json({ success: false, error: String(err) }, { status: 500 });
  }
}
