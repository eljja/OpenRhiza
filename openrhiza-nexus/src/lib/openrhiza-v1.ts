export type IdentityType = "software_key" | "tpm_key";

export type TransportCapability =
  | "tls"
  | "http_json"
  | "signed_wasm"
  | "driver_download";

export type TrustTier = "software" | "tpm";

export type BusType = "pci" | "usb" | "storage";

export interface NodeRegisterRequest {
  protocol_version: "v1";
  node_id: string;
  public_key: string;
  identity_type: IdentityType;
  tpm_present: boolean;
  os_version: string;
  transport_capabilities: TransportCapability[];
}

export interface NodeHeartbeatRequest {
  protocol_version: "v1";
  node_id: string;
  hardware_fingerprint: string;
  uptime_ms: number;
  active_driver_count: number;
  network_online: boolean;
}

export interface MachineProfile {
  cpu: {
    vendor: string;
    family: number;
    model: number;
    stepping: number;
    logical_cores: number;
  };
  memory: {
    total_bytes: number;
  };
  network: {
    mac_addresses: string[];
  };
  tpm: {
    present: boolean;
    attestation_ready: boolean;
  };
}

export interface HardwareDevice {
  bus_type: BusType;
  vendor_id: string;
  device_id: string;
  class_code?: string;
  subclass?: string;
  prog_if?: string;
  bus?: number;
  slot?: number;
}

export interface HardwareReportRequest {
  protocol_version: "v1";
  node_id: string;
  hardware_fingerprint: string;
  machine_profile: MachineProfile;
  devices: HardwareDevice[];
}

export interface DriverQueryRequest {
  protocol_version: "v1";
  node_id: string;
  devices: HardwareDevice[];
}

export interface DriverUploadRequest {
  protocol_version: "v1";
  node_id: string;
  match_key: string;
  display_name: string;
  hardware: string;
  source_type: "gemini_generated";
  model: string;
  prompt_hash: string;
  payload_text: string;
}

export interface DriverDownloadRequest {
  protocol_version: "v1";
  node_id: string;
  driver_id?: string;
  match_key?: string;
}

export interface DriverCommentRequest {
  protocol_version: "v1";
  node_id: string;
  driver_id: string;
  comment: string;
}

export interface DriverVoteRequest {
  protocol_version: "v1";
  node_id: string;
  driver_id: string;
  vote: "up" | "down";
}

export interface SkillQueryRequest {
  protocol_version: "v1";
  node_id: string;
  capabilities: string[];
  preferred_domains: string[];
}

export interface WorkflowQueryRequest {
  protocol_version: "v1";
  node_id: string;
  goal: string;
  available_skills: string[];
}

export interface PolicyQueryRequest {
  protocol_version: "v1";
  node_id: string;
  scope: "runtime" | "driver" | "storage" | "workflow" | "all";
}

export interface EvaluationQueryRequest {
  protocol_version: "v1";
  node_id: string;
  subject_type: "driver" | "skill" | "workflow" | "program" | "all";
}

export interface SoftwareQueryRequest {
  protocol_version: "v1";
  node_id: string;
  ui_mode: "cli" | "terminal" | "text_web";
  capabilities: string[];
}

export interface LlmQueryRequest {
  protocol_version: "v1";
  node_id: string;
  machine_profile: Partial<MachineProfile>;
  acceleration: {
    gpu_present: boolean;
    npu_present: boolean;
  };
}

export interface EvaluationUploadRequest {
  protocol_version: "v1";
  node_id: string;
  subject_type?: "driver" | "skill" | "workflow" | "program";
  subject_id?: string;
  subject_label?: string;
  driver_id?: string;
  hardware_match_key?: string;
  stability_score: number;
  performance_score: number;
  notes: string[];
}

export function isV1Protocol(input: unknown): boolean {
  return Boolean(
    input &&
      typeof input === "object" &&
      "protocol_version" in input &&
      (input as { protocol_version?: string }).protocol_version === "v1",
  );
}

export function ok<T>(data: T) {
  return {
    success: true as const,
    data,
  };
}

export function fail(message: string, status = 400) {
  return {
    success: false as const,
    message,
    status,
  };
}


