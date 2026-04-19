import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

import type {
  DriverCommentRequest,
  DriverQueryRequest,
  DriverUploadRequest,
  DriverVoteRequest,
  EvaluationUploadRequest,
  HardwareDevice,
  HardwareReportRequest,
  LlmQueryRequest,
  NodeHeartbeatRequest,
  NodeRegisterRequest,
  SoftwareQueryRequest,
} from "@/lib/openrhiza-v1";

export const registryVersion = "v1";

export interface DriverRecord {
  driver_id: string;
  display_name: string;
  match_key: string;
  hardware: string;
  delivery_type: string;
  stability_score: number;
  performance_score: number;
  summary: string;
  status: "verified" | "testing" | "proposed";
  improvements: string[];
  updated_at: string;
}

export interface DriverCommentRecord {
  comment_id: string;
  driver_id: string;
  node_id: string;
  comment: string;
  created_at: string;
}

export interface DriverVoteSummary {
  upvotes: number;
  downvotes: number;
  score: number;
}

export interface SoftwareRecord {
  package_id: string;
  display_name: string;
  category: string;
  delivery: string;
  summary: string;
  status: "available" | "testing" | "planned";
  updated_at: string;
}

export interface LlmRecord {
  model_id: string;
  display_name: string;
  provider: string;
  mode: string;
  summary: string;
  recommended_for: string[];
  status: "online" | "limited" | "planned";
}

export interface NodeRecord {
  node_id: string;
  trust_tier: "software" | "tpm";
  public_key: string;
  hardware_fingerprint: string;
  status: "online" | "idle" | "testing";
  note: string;
  last_seen: string;
  tpm_present: boolean;
  os_version: string;
  transport_capabilities: string[];
  machine_profile_json: string;
  devices_json: string;
}

export interface EvaluationRecord {
  evaluation_id: string;
  subject: string;
  node_id: string;
  driver_id: string;
  hardware_match_key: string;
  stability_score: number;
  performance_score: number;
  note: string;
  notes: string[];
  created_at: string;
}

let database: DatabaseSync | null = null;

function dbPath() {
  const configured = process.env.OPENRHIZA_DB_PATH;
  if (configured) {
    return configured;
  }

  return path.join(process.cwd(), "data", "openrhiza.db");
}

function parseJsonArray(value: string) {
  try {
    return JSON.parse(value) as string[];
  } catch {
    return [];
  }
}

function mapDriver(row: Record<string, unknown>): DriverRecord {
  return {
    driver_id: String(row.driver_id),
    display_name: String(row.display_name),
    match_key: String(row.match_key),
    hardware: String(row.hardware),
    delivery_type: String(row.delivery_type),
    stability_score: Number(row.stability_score),
    performance_score: Number(row.performance_score),
    summary: String(row.summary),
    status: row.status as DriverRecord["status"],
    improvements: parseJsonArray(String(row.improvements_json ?? "[]")),
    updated_at: String(row.updated_at),
  };
}

function mapDriverComment(row: Record<string, unknown>): DriverCommentRecord {
  return {
    comment_id: String(row.comment_id),
    driver_id: String(row.driver_id),
    node_id: String(row.node_id),
    comment: String(row.comment_text),
    created_at: String(row.created_at),
  };
}

function mapSoftware(row: Record<string, unknown>): SoftwareRecord {
  return {
    package_id: String(row.package_id),
    display_name: String(row.display_name),
    category: String(row.category),
    delivery: String(row.delivery),
    summary: String(row.summary),
    status: row.status as SoftwareRecord["status"],
    updated_at: String(row.updated_at),
  };
}

function mapModel(row: Record<string, unknown>): LlmRecord {
  return {
    model_id: String(row.model_id),
    display_name: String(row.display_name),
    provider: String(row.provider),
    mode: String(row.mode),
    summary: String(row.summary),
    recommended_for: parseJsonArray(String(row.recommended_for_json ?? "[]")),
    status: row.status as LlmRecord["status"],
  };
}

function mapNode(row: Record<string, unknown>): NodeRecord {
  return {
    node_id: String(row.node_id),
    trust_tier: row.trust_tier as NodeRecord["trust_tier"],
    public_key: String(row.public_key),
    hardware_fingerprint: String(row.hardware_fingerprint),
    status: row.status as NodeRecord["status"],
    note: String(row.note),
    last_seen: String(row.last_seen),
    tpm_present: Number(row.tpm_present) === 1,
    os_version: String(row.os_version),
    transport_capabilities: parseJsonArray(String(row.transport_capabilities_json ?? "[]")),
    machine_profile_json: String(row.machine_profile_json ?? "{}"),
    devices_json: String(row.devices_json ?? "[]"),
  };
}

function mapEvaluation(row: Record<string, unknown>): EvaluationRecord {
  return {
    evaluation_id: String(row.evaluation_id),
    subject: String(row.subject),
    node_id: String(row.node_id),
    driver_id: String(row.driver_id),
    hardware_match_key: String(row.hardware_match_key),
    stability_score: Number(row.stability_score),
    performance_score: Number(row.performance_score),
    note: String(row.note),
    notes: parseJsonArray(String(row.notes_json ?? "[]")),
    created_at: String(row.created_at),
  };
}

function normalizeMatchKey(device: HardwareDevice) {
  return `${device.bus_type}:${device.vendor_id}:${device.device_id}`;
}

function lookupMatchingDrivers(device: HardwareDevice) {
  const db = openDb();
  const exactKey = normalizeMatchKey(device);
  const rows = db.prepare("SELECT * FROM drivers WHERE match_key = ?").all(exactKey);
  const exactMatches = rows.map((row) => mapDriver(row as Record<string, unknown>));
  if (exactMatches.length > 0) {
    return exactMatches;
  }

  if (!device.class_code) {
    return [];
  }

  const classKey = `${device.bus_type}:class:${device.class_code}${device.subclass ?? ""}`;
  return db
    .prepare("SELECT * FROM drivers WHERE match_key = ?")
    .all(classKey)
    .map((row) => mapDriver(row as Record<string, unknown>));
}

function seedIfEmpty(db: DatabaseSync) {
  db.exec(`
    INSERT OR IGNORE INTO drivers VALUES
    ('drv_e1000_native_v1','Intel e1000 Native Driver','pci:8086:100e','Intel 82540EM / e1000','builtin_reference',92,88,'Stable baseline network driver for the standard QEMU e1000 adapter.','verified','["Validate RX ring starvation under sustained burst traffic."]','2026-04-18'),
    ('drv_xhci_native_v1','xHCI Native USB Driver','pci:8086:1e31','Generic xHCI USB Host Controller','builtin_reference',85,80,'Native USB host driver used for keyboard input and future HID expansion.','testing','["Add mouse support.","Expand multi-device handling.","Harden long-run polling stability."]','2026-04-18'),
    ('drv_ahci_candidate_v1','AHCI Storage Candidate','pci:class:0106','Generic AHCI SATA Controller','sandbox_candidate',71,69,'Candidate storage path intended for wider bare-metal validation before promotion.','proposed','["Add write-path validation.","Improve timeout recovery.","Test mixed disk geometries."]','2026-04-17'),
    ('drv_pci_hostbridge_qemu_v1','PCI Host Bridge Baseline','pci:class:0600','Generic PCI host bridge baseline for QEMU and early bare-metal boot paths.','builtin_reference',90,76,'Baseline chipset support record for PCI host bridge discovery and stable enumeration.','verified','["Capture more chipset-specific notes for real hardware."]','2026-04-19'),
    ('drv_piix_isa_bridge_v1','ISA Bridge Baseline','pci:class:0601','Generic ISA bridge baseline for QEMU PIIX/legacy compatibility paths.','builtin_reference',89,74,'Baseline compatibility record for ISA bridge presence on legacy-compatible virtual hardware.','verified','["Track interrupt routing notes for additional southbridge variants."]','2026-04-19'),
    ('drv_piix_ide_v1','PIIX IDE Baseline','pci:class:0101','Generic IDE controller baseline for QEMU PIIX storage paths.','builtin_reference',88,78,'Reference IDE controller support record for current OpenRhiza ATA read path.','verified','["Add write-path validation and wider disk geometry coverage."]','2026-04-19'),
    ('drv_stdvga_qemu_v1','Standard VGA Baseline','pci:class:0300','Generic VGA/display adapter baseline for QEMU standard VGA style adapters.','builtin_reference',87,72,'Display adapter support record for text-first OpenRhiza environments and future framebuffer work.','testing','["Expand beyond text mode and document framebuffer capabilities."]','2026-04-19'),
    ('drv_xhci_class_baseline_v1','USB xHCI Class Baseline','pci:class:0c03','Generic xHCI class baseline for USB host controller discovery and matching.','builtin_reference',85,80,'Class-based baseline for xHCI host controllers used by keyboard input and future HID devices.','testing','["Confirm exact controller variants and add mouse/HID composite coverage."]','2026-04-19');

    INSERT OR IGNORE INTO software_packages VALUES
    ('pkg_terminal_tools_v1','Terminal Starter Tools','system','text_bundle','Basic CLI-first package set for networked OpenRhiza systems.','available','2026-04-18'),
    ('pkg_diag_console_v1','Diagnostic Console','debugging','text_bundle','Operator-oriented inspection tools for hardware inventory and service API state.','testing','2026-04-18'),
    ('pkg_driver_lab_v1','Driver Lab','development','sandbox_package','Utilities for generating, validating, and promoting driver candidates from the sandbox.','planned','2026-04-16');

    INSERT OR IGNORE INTO llm_models VALUES
    ('llm_remote_general_v1','OpenRhiza Remote General Model','OpenRhiza','remote_api','General-purpose remote inference endpoint for early OpenRhiza nodes.','["driver planning","software generation","registry lookups"]','online'),
    ('llm_google_gateway_candidate','Google API Gateway Candidate','Google','remote_api','Planned external LLM integration for code generation and structured reasoning tasks.','["driver generation","program synthesis","analysis"]','planned');

    INSERT OR IGNORE INTO nodes VALUES
    ('orhiza_node_qemu_01','software','seed_public_key_demo','sha256:4fe9...b8a1','online','QEMU test node with e1000 and xHCI keyboard path.','2026-04-18T12:30:00Z',0,'0.1.0','["tls","http_json","signed_wasm"]','{}','[]'),
    ('orhiza_node_lab_02','software','seed_public_key_lab','sha256:ab12...9fd0','testing','Long-run stability validation for input and service API tasks.','2026-04-18T10:05:00Z',0,'0.1.0','["tls","http_json"]','{}','[]');

    INSERT OR IGNORE INTO evaluations VALUES
    ('eval_orhiza_node_qemu_01_drv_e1000_native_v1','Intel e1000 Native Driver','orhiza_node_qemu_01','drv_e1000_native_v1','pci:8086:100e',92,88,'Strong baseline in QEMU. Continue stress testing under repeated API fetch and long uptime.','["Strong baseline in QEMU. Continue stress testing under repeated API fetch and long uptime."]','2026-04-18'),
    ('eval_orhiza_node_lab_02_drv_xhci_native_v1','xHCI Native USB Driver','orhiza_node_lab_02','drv_xhci_native_v1','pci:8086:1e31',85,80,'Good boot and input behavior. Continue checking long-duration keyboard responsiveness.','["Good boot and input behavior. Continue checking long-duration keyboard responsiveness."]','2026-04-18');
  `);
}

function openDb() {
  if (database) {
    return database;
  }

  const filePath = dbPath();
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  database = new DatabaseSync(filePath);
  database.exec(`
    PRAGMA journal_mode = WAL;

    CREATE TABLE IF NOT EXISTS drivers (
      driver_id TEXT PRIMARY KEY,
      display_name TEXT NOT NULL,
      match_key TEXT NOT NULL UNIQUE,
      hardware TEXT NOT NULL,
      delivery_type TEXT NOT NULL,
      stability_score INTEGER NOT NULL,
      performance_score INTEGER NOT NULL,
      summary TEXT NOT NULL,
      status TEXT NOT NULL,
      improvements_json TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS driver_artifacts (
      artifact_id TEXT PRIMARY KEY,
      driver_id TEXT NOT NULL,
      node_id TEXT NOT NULL,
      source_type TEXT NOT NULL,
      model TEXT NOT NULL,
      prompt_hash TEXT NOT NULL,
      payload_text TEXT NOT NULL,
      created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS driver_comments (
      comment_id TEXT PRIMARY KEY,
      driver_id TEXT NOT NULL,
      node_id TEXT NOT NULL,
      comment_text TEXT NOT NULL,
      created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS driver_votes (
      vote_id TEXT PRIMARY KEY,
      driver_id TEXT NOT NULL,
      node_id TEXT NOT NULL,
      vote_value INTEGER NOT NULL,
      created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS software_packages (
      package_id TEXT PRIMARY KEY,
      display_name TEXT NOT NULL,
      category TEXT NOT NULL,
      delivery TEXT NOT NULL,
      summary TEXT NOT NULL,
      status TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS llm_models (
      model_id TEXT PRIMARY KEY,
      display_name TEXT NOT NULL,
      provider TEXT NOT NULL,
      mode TEXT NOT NULL,
      summary TEXT NOT NULL,
      recommended_for_json TEXT NOT NULL,
      status TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS nodes (
      node_id TEXT PRIMARY KEY,
      trust_tier TEXT NOT NULL,
      public_key TEXT NOT NULL,
      hardware_fingerprint TEXT NOT NULL DEFAULT '',
      status TEXT NOT NULL DEFAULT 'online',
      note TEXT NOT NULL DEFAULT '',
      last_seen TEXT NOT NULL,
      tpm_present INTEGER NOT NULL DEFAULT 0,
      os_version TEXT NOT NULL DEFAULT '',
      transport_capabilities_json TEXT NOT NULL DEFAULT '[]',
      machine_profile_json TEXT NOT NULL DEFAULT '{}',
      devices_json TEXT NOT NULL DEFAULT '[]'
    );

    CREATE TABLE IF NOT EXISTS evaluations (
      evaluation_id TEXT PRIMARY KEY,
      subject TEXT NOT NULL,
      node_id TEXT NOT NULL,
      driver_id TEXT NOT NULL,
      hardware_match_key TEXT NOT NULL,
      stability_score INTEGER NOT NULL,
      performance_score INTEGER NOT NULL,
      note TEXT NOT NULL,
      notes_json TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
  `);

  seedIfEmpty(database);
  return database;
}

export function listDrivers() {
  return openDb()
    .prepare("SELECT * FROM drivers ORDER BY updated_at DESC, driver_id ASC")
    .all()
    .map((row) => mapDriver(row as Record<string, unknown>));
}

export function getDriver(driverId: string) {
  const row = openDb().prepare("SELECT * FROM drivers WHERE driver_id = ?").get(driverId) as
    | Record<string, unknown>
    | undefined;
  return row ? mapDriver(row) : null;
}

export function getDriverByMatchKey(matchKey: string) {
  const row = openDb().prepare("SELECT * FROM drivers WHERE match_key = ?").get(matchKey) as
    | Record<string, unknown>
    | undefined;
  return row ? mapDriver(row) : null;
}

export function listDriverCommentsForDriver(driverId: string) {
  return openDb()
    .prepare("SELECT * FROM driver_comments WHERE driver_id = ? ORDER BY created_at DESC")
    .all(driverId)
    .map((row) => mapDriverComment(row as Record<string, unknown>));
}

export function getDriverVoteSummary(driverId: string): DriverVoteSummary {
  const row = openDb()
    .prepare(`
      SELECT
        SUM(CASE WHEN vote_value > 0 THEN 1 ELSE 0 END) AS upvotes,
        SUM(CASE WHEN vote_value < 0 THEN 1 ELSE 0 END) AS downvotes
      FROM driver_votes
      WHERE driver_id = ?
    `)
    .get(driverId) as Record<string, unknown> | undefined;

  const upvotes = Number(row?.upvotes ?? 0);
  const downvotes = Number(row?.downvotes ?? 0);
  return { upvotes, downvotes, score: upvotes - downvotes };
}

export function listSoftwarePackages() {
  return openDb()
    .prepare("SELECT * FROM software_packages ORDER BY updated_at DESC, package_id ASC")
    .all()
    .map((row) => mapSoftware(row as Record<string, unknown>));
}

export function getSoftwarePackage(packageId: string) {
  const row = openDb().prepare("SELECT * FROM software_packages WHERE package_id = ?").get(packageId) as
    | Record<string, unknown>
    | undefined;
  return row ? mapSoftware(row) : null;
}

export function listModels() {
  const models = openDb()
    .prepare("SELECT * FROM llm_models ORDER BY model_id ASC")
    .all()
    .map((row) => mapModel(row as Record<string, unknown>));

  if (process.env.GOOGLE_GEMINI_API_KEY) {
    return models.map((model) =>
      model.provider === "Google"
        ? {
            ...model,
            status: "online" as const,
          }
        : model,
    );
  }

  return models;
}

export function getModel(modelId: string) {
  const row = openDb().prepare("SELECT * FROM llm_models WHERE model_id = ?").get(modelId) as
    | Record<string, unknown>
    | undefined;
  return row ? mapModel(row) : null;
}

export function listNodes() {
  return openDb()
    .prepare("SELECT * FROM nodes ORDER BY last_seen DESC, node_id ASC")
    .all()
    .map((row) => mapNode(row as Record<string, unknown>));
}

export function getNode(nodeId: string) {
  const row = openDb().prepare("SELECT * FROM nodes WHERE node_id = ?").get(nodeId) as
    | Record<string, unknown>
    | undefined;
  return row ? mapNode(row) : null;
}

export function listEvaluations() {
  return openDb()
    .prepare("SELECT * FROM evaluations ORDER BY created_at DESC, evaluation_id ASC")
    .all()
    .map((row) => mapEvaluation(row as Record<string, unknown>));
}

export function listEvaluationsForNode(nodeId: string) {
  return openDb()
    .prepare("SELECT * FROM evaluations WHERE node_id = ? ORDER BY created_at DESC")
    .all(nodeId)
    .map((row) => mapEvaluation(row as Record<string, unknown>));
}

export function listEvaluationsForDriver(driverId: string) {
  return openDb()
    .prepare("SELECT * FROM evaluations WHERE driver_id = ? ORDER BY created_at DESC")
    .all(driverId)
    .map((row) => mapEvaluation(row as Record<string, unknown>));
}

export function registerNode(input: NodeRegisterRequest) {
  const trustTier = input.identity_type === "tpm_key" ? "tpm" : "software";
  const now = new Date().toISOString();

  openDb()
    .prepare(`
      INSERT INTO nodes (
        node_id, trust_tier, public_key, hardware_fingerprint, status, note, last_seen,
        tpm_present, os_version, transport_capabilities_json, machine_profile_json, devices_json
      ) VALUES (?, ?, ?, '', 'online', '', ?, ?, ?, ?, '{}', '[]')
      ON CONFLICT(node_id) DO UPDATE SET
        trust_tier = excluded.trust_tier,
        public_key = excluded.public_key,
        last_seen = excluded.last_seen,
        tpm_present = excluded.tpm_present,
        os_version = excluded.os_version,
        transport_capabilities_json = excluded.transport_capabilities_json
    `)
    .run(
      input.node_id,
      trustTier,
      input.public_key,
      now,
      input.tpm_present ? 1 : 0,
      input.os_version,
      JSON.stringify(input.transport_capabilities),
    );

  return {
    node_id: input.node_id,
    trust_tier: trustTier as "software" | "tpm",
  };
}

export function recordHeartbeat(input: NodeHeartbeatRequest) {
  const now = new Date().toISOString();
  openDb()
    .prepare(`
      UPDATE nodes
      SET hardware_fingerprint = ?, status = ?, last_seen = ?
      WHERE node_id = ?
    `)
    .run(input.hardware_fingerprint, input.network_online ? "online" : "idle", now, input.node_id);

  return {
    server_time: now,
    next_actions: [],
  };
}

export function recordHardwareReport(input: HardwareReportRequest) {
  const recognized = input.devices.filter((device) => lookupMatchingDrivers(device).length > 0).length;

  openDb()
    .prepare(`
      UPDATE nodes
      SET hardware_fingerprint = ?, machine_profile_json = ?, devices_json = ?, last_seen = ?
      WHERE node_id = ?
    `)
    .run(
      input.hardware_fingerprint,
      JSON.stringify(input.machine_profile),
      JSON.stringify(input.devices),
      new Date().toISOString(),
      input.node_id,
    );

  return {
    profile_id: `hwprof_${input.node_id}`,
    recognized_devices: recognized,
    unknown_devices: input.devices.length - recognized,
  };
}

export function queryDrivers(input: DriverQueryRequest) {
  const deduped = new Map<string, DriverRecord>();
  let matchedDevices = 0;

  for (const device of input.devices) {
    const matches = lookupMatchingDrivers(device);
    if (matches.length > 0) {
      matchedDevices += 1;
    }
    for (const match of matches) {
      deduped.set(match.driver_id, match);
    }
  }

  return {
    requested_devices: input.devices.length,
    matched_devices: matchedDevices,
    unmatched_devices: input.devices.length - matchedDevices,
    recommendations: [...deduped.values()].map((driver) => ({
      ...driver,
      vote_summary: getDriverVoteSummary(driver.driver_id),
    })),
  };
}

export function uploadGeneratedDriver(input: DriverUploadRequest) {
  const db = openDb();
  const now = new Date().toISOString();
  const existing = getDriverByMatchKey(input.match_key);
  const driverId = existing?.driver_id ?? `drv_generated_${input.match_key.replace(/[^a-z0-9]+/gi, "_").toLowerCase()}`;
  const artifactId = `artifact_${driverId}_${Date.now()}`;

  if (!existing) {
    db.prepare(`
      INSERT INTO drivers (
        driver_id, display_name, match_key, hardware, delivery_type, stability_score,
        performance_score, summary, status, improvements_json, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).run(
      driverId,
      input.display_name,
      input.match_key,
      input.hardware,
      input.source_type,
      55,
      55,
      `Gemini-generated candidate uploaded by ${input.node_id}. Pending sandbox validation and field feedback.`,
      "proposed",
      JSON.stringify(["Run sandbox smoke tests.", "Validate on matching hardware.", "Collect comments and votes before promotion."]),
      now,
    );
  }

  db.prepare(`
    INSERT INTO driver_artifacts (
      artifact_id, driver_id, node_id, source_type, model, prompt_hash, payload_text, created_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
  `).run(
    artifactId,
    driverId,
    input.node_id,
    input.source_type,
    input.model,
    input.prompt_hash,
    input.payload_text,
    now,
  );

  db.prepare("UPDATE drivers SET updated_at = ?, status = 'testing' WHERE driver_id = ?").run(now, driverId);

  return {
    driver_id: driverId,
    artifact_id: artifactId,
    reused_existing_driver: Boolean(existing),
  };
}

export function addDriverComment(input: DriverCommentRequest) {
  const commentId = `comment_${input.driver_id}_${Date.now()}`;
  const now = new Date().toISOString();

  openDb()
    .prepare(`
      INSERT INTO driver_comments (comment_id, driver_id, node_id, comment_text, created_at)
      VALUES (?, ?, ?, ?, ?)
    `)
    .run(commentId, input.driver_id, input.node_id, input.comment, now);

  return {
    comment_id: commentId,
    driver_id: input.driver_id,
  };
}

export function addDriverVote(input: DriverVoteRequest) {
  const voteId = `vote_${input.driver_id}_${input.node_id}_${Date.now()}`;
  const now = new Date().toISOString();
  const voteValue = input.vote === "up" ? 1 : -1;

  openDb()
    .prepare(`
      INSERT INTO driver_votes (vote_id, driver_id, node_id, vote_value, created_at)
      VALUES (?, ?, ?, ?, ?)
    `)
    .run(voteId, input.driver_id, input.node_id, voteValue, now);

  return getDriverVoteSummary(input.driver_id);
}

export function querySoftware(_input: SoftwareQueryRequest) {
  return {
    packages: listSoftwarePackages(),
  };
}

export function queryModels(_input: LlmQueryRequest) {
  return {
    models: listModels(),
  };
}

export function recordEvaluation(input: EvaluationUploadRequest) {
  const evaluationId = `eval_${input.node_id}_${input.driver_id}_${Date.now()}`;
  const driver = getDriver(input.driver_id);
  const note = input.notes[0] ?? "No detailed note provided.";
  const createdAt = new Date().toISOString();

  openDb()
    .prepare(`
      INSERT INTO evaluations (
        evaluation_id, subject, node_id, driver_id, hardware_match_key,
        stability_score, performance_score, note, notes_json, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `)
    .run(
      evaluationId,
      driver?.display_name ?? input.driver_id,
      input.node_id,
      input.driver_id,
      input.hardware_match_key,
      input.stability_score,
      input.performance_score,
      note,
      JSON.stringify(input.notes),
      createdAt,
    );

  return {
    evaluation_id: evaluationId,
  };
}

export function searchDriverByLegacyHardwareId(hardwareId: string) {
  const normalized = hardwareId.trim().toLowerCase();
  const exactKey = `pci:${normalized.replace(":", ":")}`;
  return listDrivers().find((driver) => driver.match_key === exactKey) ?? null;
}

export function archiveUploadedDriver(input: {
  node_id?: string;
  hardware_id?: string;
  hardware_name?: string;
  display_name?: string;
  code_snippet?: string;
  warnings?: string;
}) {
  const normalizedHardwareId = (input.hardware_id ?? "custom:unknown").toLowerCase();
  const uploaded = uploadGeneratedDriver({
    protocol_version: "v1",
    node_id: input.node_id ?? "legacy_upload",
    match_key: `pci:${normalizedHardwareId}`,
    display_name: input.display_name ?? input.hardware_name ?? `Uploaded Driver ${normalizedHardwareId}`,
    hardware: input.hardware_name ?? normalizedHardwareId,
    source_type: "gemini_generated",
    model: "legacy_upload",
    prompt_hash: "legacy_upload",
    payload_text: input.code_snippet ?? "",
  });

  if (input.warnings) {
    addDriverComment({
      protocol_version: "v1",
      node_id: input.node_id ?? "legacy_upload",
      driver_id: uploaded.driver_id,
      comment: input.warnings,
    });
  }

  return {
    id: uploaded.driver_id,
    message: "Driver payload archived in Nexus.",
  };
}
