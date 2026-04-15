import { NextResponse } from "next/server";
import { fail, isV1Protocol, ok, type DriverQueryRequest } from "@/lib/openrhiza-v1";

export async function POST(req: Request) {
  try {
    const body = (await req.json()) as DriverQueryRequest;

    if (!isV1Protocol(body)) {
      return NextResponse.json(fail("Unsupported protocol version."), { status: 400 });
    }

    if (!body.node_id) {
      return NextResponse.json(fail("node_id is required."), { status: 400 });
    }

    const recommendations = body.devices.flatMap((device) => {
      if (device.bus_type === "pci" && device.vendor_id === "8086" && device.device_id === "100e") {
        return [
          {
            match_key: "pci:8086:100e",
            driver_id: "drv_e1000_native_v1",
            display_name: "Intel e1000 Native Driver",
            delivery_type: "builtin_reference",
            stability_score: 92,
            performance_score: 88,
            summary: "Recommended for standard Intel e1000 adapters.",
            improvements: ["Validate RX ring starvation under sustained burst traffic."],
          },
        ];
      }

      if (device.bus_type === "pci" && device.vendor_id === "8086" && device.class_code === "0c") {
        return [
          {
            match_key: `pci:${device.vendor_id}:${device.device_id}`,
            driver_id: "drv_xhci_native_v1",
            display_name: "xHCI Native USB Driver",
            delivery_type: "builtin_reference",
            stability_score: 85,
            performance_score: 80,
            summary: "Recommended for standard xHCI USB host controllers.",
            improvements: ["Add mouse and composite HID coverage.", "Expand multi-device validation."],
          },
        ];
      }

      return [];
    });

    return NextResponse.json(ok({ recommendations }));
  } catch (error) {
    return NextResponse.json(fail(String(error), 500), { status: 500 });
  }
}
