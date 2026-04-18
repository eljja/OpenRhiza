import { NextResponse } from "next/server";
import { searchDriverByLegacyHardwareId } from "@/app/registry-data";

export async function GET(req: Request) {
  const { searchParams } = new URL(req.url);
  const hardwareId = searchParams.get("hw_id");

  if (!hardwareId) {
    return NextResponse.json({ success: false, message: "hw_id is required." }, { status: 400 });
  }

  const driver = searchDriverByLegacyHardwareId(hardwareId);
  if (!driver) {
    return NextResponse.json({
      success: false,
      message: "No certified driver found for this HW_ID. You must generate one locally.",
    });
  }

  return NextResponse.json({
    success: true,
    data: {
      hardware_id: hardwareId,
      hardware_name: driver.hardware,
      driver_id: driver.driver_id,
      display_name: driver.display_name,
      stability_score: driver.stability_score,
      performance_score: driver.performance_score,
      summary: driver.summary,
      warnings: driver.improvements.join(" "),
    },
  });
}
