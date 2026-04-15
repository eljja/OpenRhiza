import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type HardwareReportRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as HardwareReportRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id || !body.hardware_fingerprint) {
      return NextResponse.json(fail("node_id and hardware_fingerprint are required."), { status: 400 });
    }

    const recognizedDevices = body.devices.filter(
      (device) => device.bus_type === "pci" && device.vendor_id === "8086" && device.device_id === "100e",
    ).length;

    return NextResponse.json(
      ok({
        profile_id: `hwprof_${body.node_id}`,
        recognized_devices: recognizedDevices,
        unknown_devices: body.devices.length - recognizedDevices,
      }),
    );
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
