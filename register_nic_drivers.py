#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
register_nic_drivers.py
OpenRhiza NIC Driver Auto-Registration Script

Reads driver_manifest.json, runs simulation tests for each driver,
and registers results to the OpenRhiza Nexus API.

Usage:
  python register_nic_drivers.py             # register to OPENRHIZA_BASE_URL (default: localhost:3000)
  python register_nic_drivers.py --dry-run   # print payloads without sending
  python register_nic_drivers.py --test-only # run tests, skip registration

Environment Variables:
  OPENRHIZA_BASE_URL   Base URL of the Nexus server (default: http://localhost:3000)
  OPENRHIZA_NODE_ID    Node ID for registration (default: auto-generated)
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from typing import Optional

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

DEFAULT_BASE_URL = "http://localhost:3000"
MANIFEST_PATH = os.path.join(os.path.dirname(__file__), "nic_drivers", "driver_manifest.json")
TEST_MODULE_DIR = os.path.join(os.path.dirname(__file__), "nic_drivers", "tests")

PROTOCOL_VERSION = "v1"
NODE_ID = os.environ.get("OPENRHIZA_NODE_ID", "driver_pipeline_node_01")
MODEL_NAME = "driver_pipeline_v1"

# ---------------------------------------------------------------------------
# API helpers
# ---------------------------------------------------------------------------

def api_post(base_url: str, path: str, payload: dict, dry_run: bool) -> Optional[dict]:
    url = base_url.rstrip('/') + path
    body = json.dumps(payload).encode('utf-8')

    if dry_run:
        print(f"\n  [DRY-RUN] POST {url}")
        print(f"  Payload ({len(body)} bytes):")
        print("  " + json.dumps(payload, indent=4).replace('\n', '\n  '))
        return {"success": True, "dry_run": True}

    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Content-Type": "application/json",
            "User-Agent": "OpenRhiza-NIC-Driver-Pipeline/1.0",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            raw = resp.read()
            return json.loads(raw)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return json.loads(raw)
        except Exception:
            print(f"  [ERROR] HTTP {e.code} from {url}: {raw[:200]}")
            return None
    except Exception as exc:
        print(f"  [ERROR] Connection failed to {url}: {exc}")
        return None


def register_node(base_url: str, dry_run: bool) -> bool:
    payload = {
        "protocol_version": PROTOCOL_VERSION,
        "node_id": NODE_ID,
        "public_key": "pipeline_synthetic_key_01",
        "identity_type": "software_key",
        "tpm_present": False,
        "os_version": "0.1.0",
        "transport_capabilities": ["http_json"],
    }
    print(f"\n[Step 1] Registering pipeline node as '{NODE_ID}'...")
    resp = api_post(base_url, "/api/v1/node/register", payload, dry_run)
    if resp and resp.get("success"):
        print(f"  [OK] Node registered.")
        return True
    else:
        print(f"  [WARN] Node registration response: {resp}")
        return False


def upload_driver(base_url: str, driver: dict, payload_text: str, dry_run: bool) -> Optional[str]:
    prompt_text = f"Native {driver['display_name']} candidate for OpenRhiza OS bare-metal kernel."
    prompt_hash = "sha256:" + hashlib.sha256(prompt_text.encode()).hexdigest()

    payload = {
        "protocol_version": PROTOCOL_VERSION,
        "node_id": NODE_ID,
        "match_key": driver["match_key"],
        "display_name": driver["display_name"],
        "hardware": driver["hardware"],
        "source_type": "crafted_native",
        "model": MODEL_NAME,
        "prompt_hash": prompt_hash,
        "payload_text": payload_text,
    }
    resp = api_post(base_url, "/api/v1/driver/upload", payload, dry_run)
    if resp and resp.get("success"):
        driver_id = resp.get("data", {}).get("driver_id") or driver["driver_id"]
        print(f"  [OK] Driver uploaded: {driver_id}")
        return driver_id
    elif dry_run:
        return driver["driver_id"]
    else:
        print(f"  [WARN] Upload response: {resp}")
        return driver["driver_id"]


def upload_evaluation(
    base_url: str,
    driver_id: str,
    driver: dict,
    stability: int,
    performance: int,
    notes: list[str],
    dry_run: bool,
):
    payload = {
        "protocol_version": PROTOCOL_VERSION,
        "node_id": NODE_ID,
        "subject_type": "driver",
        "subject_id": driver_id,
        "subject_label": driver["display_name"],
        "driver_id": driver_id,
        "hardware_match_key": driver["match_key"],
        "stability_score": stability,
        "performance_score": performance,
        "notes": notes,
    }
    resp = api_post(base_url, "/api/v1/evaluation/upload", payload, dry_run)
    if resp and resp.get("success"):
        print(f"  [OK] Evaluation uploaded (stability={stability}, perf={performance})")
    else:
        print(f"  [WARN] Evaluation response: {resp}")


def add_comment(base_url: str, driver_id: str, comment: str, dry_run: bool):
    payload = {
        "protocol_version": PROTOCOL_VERSION,
        "node_id": NODE_ID,
        "driver_id": driver_id,
        "comment": comment,
    }
    resp = api_post(base_url, "/api/v1/driver/comment", payload, dry_run)
    if resp and resp.get("success"):
        print(f"  [OK] Comment added.")
    else:
        print(f"  [WARN] Comment response: {resp}")


def vote_driver(base_url: str, driver_id: str, vote: str, dry_run: bool):
    payload = {
        "protocol_version": PROTOCOL_VERSION,
        "node_id": NODE_ID,
        "driver_id": driver_id,
        "vote": vote,
    }
    resp = api_post(base_url, "/api/v1/driver/vote", payload, dry_run)
    if resp and resp.get("success"):
        print(f"  [OK] Vote '{vote}' recorded.")
    else:
        print(f"  [WARN] Vote response: {resp}")


# ---------------------------------------------------------------------------
# Test runner
# ---------------------------------------------------------------------------

def run_tests_for_driver(driver: dict) -> dict:
    """
    Import and run the test module for the given driver.
    Returns the summary dict from suite.summary().
    """
    source = driver.get("test_module_override") or driver["source_file"].replace(".rs", "")
    test_module_name = f"test_{source}"
    test_file = os.path.join(TEST_MODULE_DIR, f"{test_module_name}.py")

    if not os.path.exists(test_file):
        print(f"  [WARN] No test file found at {test_file}, skipping tests.")
        return {
            "driver": driver["driver_id"],
            "passed": 0, "total": 0, "failed": 0,
            "stability_score": driver["stability_score"],
            "performance_score": driver["performance_score"],
            "all_passed": False,
            "skipped": True,
        }

    # Dynamically import the test module
    if TEST_MODULE_DIR not in sys.path:
        sys.path.insert(0, TEST_MODULE_DIR)

    import importlib
    try:
        mod = importlib.import_module(test_module_name)
        importlib.reload(mod)  # ensure fresh state
        return mod.run_all()
    except Exception as exc:
        print(f"  [ERROR] Test module {test_module_name} raised: {exc}")
        import traceback; traceback.print_exc()
        return {
            "driver": driver["driver_id"],
            "passed": 0, "total": 0, "failed": 0,
            "stability_score": 0, "performance_score": 0,
            "all_passed": False,
        }


def load_source_text(driver: dict) -> str:
    """Read the Rust source file for the driver as the payload_text."""
    src = os.path.join(os.path.dirname(__file__), "nic_drivers", driver["source_file"])
    if os.path.exists(src):
        with open(src, 'r', encoding='utf-8') as f:
            return f.read()
    return f"// Source not found: {driver['source_file']}"


# ---------------------------------------------------------------------------
# Main pipeline
# ---------------------------------------------------------------------------

def run_pipeline(base_url: str, dry_run: bool, test_only: bool):
    print(f"{'='*60}")
    print(f"OpenRhiza NIC Driver Registration Pipeline")
    print(f"  Target: {base_url}")
    print(f"  Dry-run: {dry_run} | Test-only: {test_only}")
    print(f"  Node:   {NODE_ID}")
    print(f"{'='*60}")

    # Load manifest
    with open(MANIFEST_PATH, 'r', encoding='utf-8') as f:
        manifest = json.load(f)

    drivers = manifest["drivers"]
    print(f"\nFound {len(drivers)} drivers in manifest.")

    # Register pipeline node
    if not test_only:
        register_node(base_url, dry_run)

    results = []
    for driver in drivers:
        print(f"\n{'─'*50}")
        print(f"Driver: {driver['display_name']}")
        print(f"  Match key: {driver['match_key']} | Speed: {driver['speed']}")

        # Run simulation tests
        print(f"\n  [Tests] Running simulation for {driver['source_file']}...")
        test_result = run_tests_for_driver(driver)
        results.append((driver, test_result))

        if test_only:
            continue

        # Load source payload
        payload_text = load_source_text(driver)
        print(f"\n  [Register] Uploading driver ({len(payload_text)} chars)...")
        driver_id = upload_driver(base_url, driver, payload_text, dry_run)

        if not driver_id:
            print(f"  [WARN] Skipping evaluation for {driver['driver_id']}")
            continue

        # Evaluation
        stability  = test_result.get("stability_score",  driver["stability_score"])
        performance = test_result.get("performance_score", driver["performance_score"])
        notes_base = driver.get("improvements", [])
        test_summary = (
            f"Simulation tests: {test_result['passed']}/{test_result['total']} passed. "
            f"Stability={stability}, Performance={performance}."
        )
        notes = [test_summary] + notes_base[:3]

        print(f"  [Evaluate] Uploading evaluation...")
        upload_evaluation(base_url, driver_id, driver, stability, performance, notes, dry_run)

        # Comment with test summary
        comment = (
            f"Auto-registered by OpenRhiza NIC driver pipeline. "
            f"{test_result['passed']}/{test_result['total']} simulation tests passed. "
            f"Covers: {', '.join(driver.get('pci_devices', [driver['match_key']]))}"
        )
        print(f"  [Comment] Adding auto-comment...")
        add_comment(base_url, driver_id, comment, dry_run)

        # Vote UP if all tests passed
        if test_result.get("all_passed"):
            print(f"  [Vote] All tests passed -- voting UP.")
            vote_driver(base_url, driver_id, "up", dry_run)
        else:
            print(f"  [Vote] Some tests failed -- no auto-vote.")

        time.sleep(0.2)  # be polite to the server

    # Final summary
    print(f"\n{'='*60}")
    print("Pipeline Complete - Summary:")
    print(f"{'='*60}")
    total_pass = sum(r["passed"] for _, r in results)
    total_tests = sum(r["total"] for _, r in results)
    all_ok = all(r["all_passed"] for _, r in results)

    for driver, r in results:
        status = "[PASS]" if r["all_passed"] else "[FAIL]"
        print(f"  {status} {driver['display_name']:45s}  {r['passed']}/{r['total']} tests")

    print(f"\n  Total: {total_pass}/{total_tests} tests passed across {len(drivers)} drivers.")
    print(f"  All passed: {all_ok}")
    if not test_only:
        mode = "DRY-RUN" if dry_run else "LIVE"
        print(f"  Registration mode: {mode}")
    return 0 if all_ok else 1


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="OpenRhiza NIC Driver Auto-Registration Pipeline"
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Print API payloads without sending requests."
    )
    parser.add_argument(
        "--test-only", action="store_true",
        help="Run simulation tests only, skip API registration."
    )
    parser.add_argument(
        "--base-url", default=None,
        help="Override the Nexus server URL (overrides OPENRHIZA_BASE_URL env var)."
    )
    args = parser.parse_args()

    base_url = args.base_url or os.environ.get("OPENRHIZA_BASE_URL", DEFAULT_BASE_URL)
    exit_code = run_pipeline(base_url, args.dry_run, args.test_only)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
