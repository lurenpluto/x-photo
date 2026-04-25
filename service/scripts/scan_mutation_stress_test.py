#!/usr/bin/env python3
import argparse
import json
import os
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_DIR = SCRIPT_DIR.parent.parent
SERVICE_DIR = REPO_DIR / "service"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run scan mutation stress checks against an isolated service."
    )
    parser.add_argument("--data-root", default="/home/bucky/.xphoto/test_data")
    parser.add_argument("--profile", default="stress")
    parser.add_argument("--scenario", default="family-full-2020")
    parser.add_argument("--report-dir", default=str(REPO_DIR / "doc" / "reports"))
    parser.add_argument("--label", default=time.strftime("%Y%m%d-%H%M%S"))
    parser.add_argument("--bind-addr", default="127.0.0.1:58091")
    parser.add_argument("--timeout-sec", type=int, default=3600)
    parser.add_argument("--delete-count", type=int, default=25)
    parser.add_argument("--append-count", type=int, default=10)
    parser.add_argument("--cancel-delay-sec", type=float, default=0.2)
    parser.add_argument("--in-scan-delete-count", type=int, default=25)
    parser.add_argument("--in-scan-append-count", type=int, default=10)
    parser.add_argument("--in-scan-mutation-delay-sec", type=float, default=0.1)
    return parser.parse_args()


class StressRunner:
    def __init__(self, args: argparse.Namespace) -> None:
        self.args = args
        self.base_url = f"http://{args.bind_addr}/rpc/v1"
        self.report_dir = Path(args.report_dir)
        self.run_dir = Path(tempfile.mkdtemp(prefix=f"xphoto_scan_mutation_{args.label}."))
        self.resp_dir = self.run_dir / "responses"
        self.resp_dir.mkdir(parents=True, exist_ok=True)
        self.db_path = self.run_dir / "mutation.db"
        self.log_dir = self.run_dir / "logs"
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.service_proc: subprocess.Popen[bytes] | None = None
        self.jobs: dict[str, dict[str, object]] = {}
        self.checks: dict[str, bool] = {}
        self.metrics: dict[str, object] = {}

    def cleanup(self) -> None:
        if self.service_proc and self.service_proc.poll() is None:
            self.service_proc.terminate()
            try:
                self.service_proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.service_proc.kill()
                self.service_proc.wait(timeout=10)

    def start_service(self) -> None:
        config_path = self.run_dir / "config.toml"
        shutil.copy2(REPO_DIR / "doc" / "config.example.toml", config_path)
        env = os.environ.copy()
        env.update(
            {
                "CONFIG_PATH": str(config_path),
                "BIND_ADDR": self.args.bind_addr,
                "DATABASE_URL": f"sqlite://{self.db_path}",
                "LOG_DIR": str(self.log_dir),
                "SCAN_SOURCE_CHANGE_DETECT_ENABLED": "false",
            }
        )
        stdout = (self.run_dir / "service.stdout.log").open("wb")
        stderr = (self.run_dir / "service.stderr.log").open("wb")
        print(f"starting isolated service at {self.args.bind_addr}")
        self.service_proc = subprocess.Popen(
            ["cargo", "run", "--bin", "service"],
            cwd=SERVICE_DIR,
            env=env,
            stdout=stdout,
            stderr=stderr,
        )

        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            try:
                health = self.call_api("GET", "/health")
                if health.get("code") == 0:
                    return
            except Exception:
                pass
            if self.service_proc.poll() is not None:
                raise RuntimeError(f"service exited early; logs: {self.run_dir}")
            time.sleep(1)
        raise RuntimeError(f"service did not become healthy; logs: {self.run_dir}")

    def call_api(
        self, method: str, path: str, body: dict[str, object] | None = None
    ) -> dict[str, object]:
        data = None
        headers = {}
        if body is not None:
            data = json.dumps(body, ensure_ascii=False).encode("utf-8")
            headers["content-type"] = "application/json"
        req = urllib.request.Request(
            f"{self.base_url}{path}", data=data, method=method, headers=headers
        )
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                payload = resp.read().decode("utf-8")
        except urllib.error.HTTPError as err:
            payload = err.read().decode("utf-8")
        return json.loads(payload)

    def write_response(self, name: str, payload: dict[str, object]) -> None:
        (self.resp_dir / f"{name}.json").write_text(
            json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8"
        )

    def create_source(self, name: str, root_path: Path) -> str:
        resp = self.call_api(
            "POST",
            "/sources",
            {"name": name, "root_path": str(root_path), "source_type": "local_fs"},
        )
        self.write_response(f"source_{name}", resp)
        if resp.get("code") != 0:
            raise RuntimeError(f"create source failed: {resp}")
        return str(resp["data"]["id"])

    def trigger_scan(self, source_id: str, label: str) -> str:
        resp = self.call_api("POST", f"/sources/{source_id}/scan", {})
        self.write_response(f"trigger_{label}", resp)
        if resp.get("code") != 0:
            raise RuntimeError(f"trigger scan failed for {label}: {resp}")
        return str(resp["data"]["job_id"])

    def retry_scan(self, job_id: str, label: str) -> str:
        resp = self.call_api("POST", f"/scan-jobs/{job_id}/retry", {})
        self.write_response(f"retry_{label}", resp)
        if resp.get("code") != 0:
            raise RuntimeError(f"retry scan failed for {label}: {resp}")
        return str(resp["data"]["job_id"])

    def cancel_scan(self, job_id: str, label: str) -> dict[str, object]:
        resp = self.call_api("POST", f"/scan-jobs/{job_id}/cancel", {})
        self.write_response(f"cancel_{label}", resp)
        return resp

    def wait_job(self, job_id: str, label: str) -> dict[str, object]:
        deadline = time.monotonic() + self.args.timeout_sec
        last: dict[str, object] | None = None
        while time.monotonic() < deadline:
            resp = self.call_api("GET", f"/scan-jobs/{job_id}")
            self.write_response(f"job_{label}", resp)
            last = resp
            if resp.get("code") == 0:
                status = str(resp["data"]["status"])
                if status in {"success", "failed", "cancelled"}:
                    self.jobs[label] = resp["data"]
                    return resp["data"]
            time.sleep(1)
        raise RuntimeError(f"scan job timeout for {label}: {last}")

    def wait_job_running_or_terminal(
        self, job_id: str, label: str, timeout_sec: int = 30
    ) -> dict[str, object]:
        deadline = time.monotonic() + timeout_sec
        last: dict[str, object] | None = None
        while time.monotonic() < deadline:
            resp = self.call_api("GET", f"/scan-jobs/{job_id}")
            self.write_response(f"job_{label}_pre_mutation", resp)
            last = resp
            if resp.get("code") == 0:
                status = str(resp["data"]["status"])
                if status in {"running", "success", "failed", "cancelled"}:
                    return resp["data"]
            time.sleep(0.2)
        raise RuntimeError(f"scan job did not start for {label}: {last}")

    def search_total(self, source_id: str) -> int:
        resp = self.call_api(
            "POST",
            "/photos/search",
            {"source_id": source_id, "page": 1, "page_size": 1},
        )
        if resp.get("code") != 0:
            raise RuntimeError(f"photo search failed: {resp}")
        return int(resp["data"]["total"])

    def source_metrics(self, source_id: str) -> dict[str, int]:
        with sqlite3.connect(self.db_path) as conn:
            cur = conn.cursor()

            def scalar(sql: str, *params: str) -> int:
                return int(cur.execute(sql, params).fetchone()[0])

            return {
                "active_photos": scalar(
                    "SELECT COUNT(1) FROM photos WHERE source_id = ? AND deleted_at IS NULL",
                    source_id,
                ),
                "deleted_photos": scalar(
                    "SELECT COUNT(1) FROM photos WHERE source_id = ? AND deleted_at IS NOT NULL",
                    source_id,
                ),
                "fts_rows": scalar(
                    """
                    SELECT COUNT(1)
                    FROM photo_search_fts f
                    INNER JOIN photos p ON p.id = f.photo_id
                    WHERE p.source_id = ? AND p.deleted_at IS NULL
                    """,
                    source_id,
                ),
                "photo_albums": scalar(
                    """
                    SELECT COUNT(1)
                    FROM photo_albums pa
                    INNER JOIN photos p ON p.id = pa.photo_id
                    WHERE p.source_id = ? AND p.deleted_at IS NULL
                    """,
                    source_id,
                ),
            }

    def global_metrics(self) -> dict[str, int]:
        with sqlite3.connect(self.db_path) as conn:
            cur = conn.cursor()

            def scalar(sql: str) -> int:
                return int(cur.execute(sql).fetchone()[0])

            return {
                "sources": scalar("SELECT COUNT(1) FROM sources"),
                "active_photos": scalar(
                    "SELECT COUNT(1) FROM photos WHERE deleted_at IS NULL"
                ),
                "deleted_photos": scalar(
                    "SELECT COUNT(1) FROM photos WHERE deleted_at IS NOT NULL"
                ),
                "fts_rows": scalar("SELECT COUNT(1) FROM photo_search_fts"),
                "duplicate_storage_groups": scalar(
                    """
                    SELECT COUNT(1)
                    FROM (
                        SELECT source_id, storage_file_id, COUNT(1) AS c
                        FROM photos
                        GROUP BY source_id, storage_file_id
                        HAVING c > 1
                    )
                    """
                ),
            }

    def run(self) -> dict[str, object]:
        self.start_service()
        mutation_root = self.prepare_mutation_data()
        baseline_files = list_photos(mutation_root)
        baseline_count = len(baseline_files)
        delete_count = min(self.args.delete_count, max(1, baseline_count // 20))
        append_count = min(self.args.append_count, max(1, baseline_count // 20))

        mutation_source_id = self.create_source(
            f"mutation-{self.args.label}", mutation_root
        )
        initial_job = self.trigger_scan(mutation_source_id, "mutation_initial")
        initial = self.wait_job(initial_job, "mutation_initial")
        initial_metrics = self.source_metrics(mutation_source_id)
        initial_api_total = self.search_total(mutation_source_id)

        delete_targets = baseline_files[:delete_count]
        for path in delete_targets:
            path.unlink()
        delete_job = self.trigger_scan(mutation_source_id, "mutation_delete")
        delete_result = self.wait_job(delete_job, "mutation_delete")
        after_delete_metrics = self.source_metrics(mutation_source_id)
        after_delete_api_total = self.search_total(mutation_source_id)

        append_dir = mutation_root / "_mutation_append"
        append_dir.mkdir(parents=True, exist_ok=True)
        remaining_files = list_photos(mutation_root)
        for idx, src in enumerate(remaining_files[:append_count], start=1):
            shutil.copy2(src, append_dir / f"appended_{idx:05d}{src.suffix.lower()}")
        append_job = self.trigger_scan(mutation_source_id, "mutation_append")
        append_result = self.wait_job(append_job, "mutation_append")
        after_append_metrics = self.source_metrics(mutation_source_id)
        after_append_api_total = self.search_total(mutation_source_id)

        cancel_source_id = self.create_source(
            f"cancel-{self.args.label}", Path(self.args.data_root)
        )
        cancel_job = self.trigger_scan(cancel_source_id, "cancel_load")
        time.sleep(self.args.cancel_delay_sec)
        cancel_resp = self.cancel_scan(cancel_job, "cancel_load")
        cancel_result = self.wait_job(cancel_job, "cancel_load")

        retry_root = self.run_dir / "retry_missing"
        retry_source_id = self.create_source(f"retry-{self.args.label}", retry_root)
        failed_job = self.trigger_scan(retry_source_id, "retry_missing_initial")
        failed_result = self.wait_job(failed_job, "retry_missing_initial")
        retry_root.mkdir(parents=True, exist_ok=True)
        shutil.copy2(remaining_files[0], retry_root / f"retry_seed{remaining_files[0].suffix}")
        retry_job = self.retry_scan(failed_job, "retry_missing")
        retry_result = self.wait_job(retry_job, "retry_missing")
        retry_metrics = self.source_metrics(retry_source_id)
        retry_api_total = self.search_total(retry_source_id)

        in_scan_result = self.run_in_scan_mutation_case()

        expected_after_delete = baseline_count - delete_count
        expected_after_append = expected_after_delete + append_count

        self.metrics = {
            "baseline_count": baseline_count,
            "delete_count": delete_count,
            "append_count": append_count,
            "expected_after_delete": expected_after_delete,
            "expected_after_append": expected_after_append,
            "initial": {
                "api_total": initial_api_total,
                "db": initial_metrics,
            },
            "after_delete": {
                "api_total": after_delete_api_total,
                "db": after_delete_metrics,
            },
            "after_append": {
                "api_total": after_append_api_total,
                "db": after_append_metrics,
            },
            "cancel": {
                "cancel_response": cancel_resp,
                "job": cancel_result,
            },
            "retry": {
                "failed_job": failed_result,
                "retry_job": retry_result,
                "api_total": retry_api_total,
                "db": retry_metrics,
            },
            "in_scan_mutation": in_scan_result,
            "global": self.global_metrics(),
        }
        self.checks = {
            "initial_scan_success": initial["status"] == "success",
            "initial_counts_match": initial_metrics["active_photos"] == baseline_count
            and initial_api_total == baseline_count
            and initial_metrics["fts_rows"] == baseline_count,
            "delete_rescan_success": delete_result["status"] == "success",
            "delete_counts_match": after_delete_metrics["active_photos"]
            == expected_after_delete
            and after_delete_api_total == expected_after_delete
            and after_delete_metrics["deleted_photos"] == delete_count
            and after_delete_metrics["fts_rows"] == expected_after_delete,
            "append_rescan_success": append_result["status"] == "success",
            "append_counts_match": after_append_metrics["active_photos"]
            == expected_after_append
            and after_append_api_total == expected_after_append
            and after_append_metrics["deleted_photos"] == delete_count
            and after_append_metrics["fts_rows"] == expected_after_append,
            "cancel_under_load_reaches_cancelled": cancel_result["status"] == "cancelled",
            "retry_initial_failure": failed_result["status"] == "failed",
            "retry_after_fix_success": retry_result["status"] == "success",
            "retry_counts_match": retry_metrics["active_photos"] == 1
            and retry_api_total == 1
            and retry_metrics["fts_rows"] == 1,
            "in_scan_baseline_counts_match": in_scan_result["baseline"]["db"][
                "active_photos"
            ]
            == in_scan_result["baseline_count"]
            and in_scan_result["baseline"]["api_total"]
            == in_scan_result["baseline_count"]
            and in_scan_result["baseline"]["db"]["fts_rows"]
            == in_scan_result["baseline_count"],
            "in_scan_mutated_while_running": in_scan_result["pre_mutation_status"]
            == "running",
            "in_scan_mutation_job_success": in_scan_result["mutation_job"]["status"]
            == "success",
            "in_scan_converge_success": in_scan_result["converge_job"]["status"]
            == "success",
            "in_scan_converge_counts_match": in_scan_result["converged"]["db"][
                "active_photos"
            ]
            == in_scan_result["expected_after_converge"]
            and in_scan_result["converged"]["api_total"]
            == in_scan_result["expected_after_converge"]
            and in_scan_result["converged"]["db"]["deleted_photos"]
            == in_scan_result["delete_count"]
            and in_scan_result["converged"]["db"]["fts_rows"]
            == in_scan_result["expected_after_converge"],
            "duplicate_storage_groups_zero": self.metrics["global"][
                "duplicate_storage_groups"
            ]
            == 0,
        }
        return self.write_report()

    def run_in_scan_mutation_case(self) -> dict[str, object]:
        root = self.prepare_in_scan_mutation_data()
        files = list_photos(root)
        baseline_count = len(files)
        delete_count = min(self.args.in_scan_delete_count, max(1, baseline_count // 20))
        append_count = min(self.args.in_scan_append_count, max(1, baseline_count // 20))

        source_id = self.create_source(f"in-scan-{self.args.label}", root)
        baseline_job_id = self.trigger_scan(source_id, "in_scan_baseline")
        baseline_job = self.wait_job(baseline_job_id, "in_scan_baseline")
        baseline_metrics = self.source_metrics(source_id)
        baseline_api_total = self.search_total(source_id)

        mutation_job_id = self.trigger_scan(source_id, "in_scan_mutation")
        pre_mutation = self.wait_job_running_or_terminal(
            mutation_job_id, "in_scan_mutation"
        )
        if pre_mutation["status"] == "running":
            time.sleep(self.args.in_scan_mutation_delay_sec)

        delete_targets = files[:delete_count]
        for path in delete_targets:
            path.unlink(missing_ok=True)

        append_dir = root / "_in_scan_append"
        append_dir.mkdir(parents=True, exist_ok=True)
        append_sources = [path for path in files if path not in set(delete_targets)]
        for idx, src in enumerate(append_sources[:append_count], start=1):
            shutil.copy2(src, append_dir / f"in_scan_appended_{idx:05d}{src.suffix.lower()}")

        mutation_job = self.wait_job(mutation_job_id, "in_scan_mutation")
        after_mutation_metrics = self.source_metrics(source_id)
        after_mutation_api_total = self.search_total(source_id)

        converge_job_id = self.trigger_scan(source_id, "in_scan_converge")
        converge_job = self.wait_job(converge_job_id, "in_scan_converge")
        converged_metrics = self.source_metrics(source_id)
        converged_api_total = self.search_total(source_id)

        return {
            "source_id": source_id,
            "baseline_count": baseline_count,
            "delete_count": delete_count,
            "append_count": append_count,
            "expected_after_converge": baseline_count - delete_count + append_count,
            "pre_mutation_status": pre_mutation["status"],
            "baseline_job": baseline_job,
            "mutation_job": mutation_job,
            "converge_job": converge_job,
            "baseline": {
                "api_total": baseline_api_total,
                "db": baseline_metrics,
            },
            "after_mutation_job": {
                "api_total": after_mutation_api_total,
                "db": after_mutation_metrics,
            },
            "converged": {
                "api_total": converged_api_total,
                "db": converged_metrics,
            },
        }

    def prepare_mutation_data(self) -> Path:
        src = Path(self.args.data_root) / self.args.scenario
        if not src.is_dir():
            raise RuntimeError(f"scenario directory not found: {src}")
        dst = self.run_dir / "mutation_data" / self.args.scenario
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(src, dst, copy_function=hardlink_or_copy)
        return dst

    def prepare_in_scan_mutation_data(self) -> Path:
        src = Path(self.args.data_root)
        if not src.is_dir():
            raise RuntimeError(f"data root not found: {src}")
        dst = self.run_dir / "in_scan_mutation_data"
        shutil.copytree(
            src,
            dst,
            copy_function=hardlink_or_copy,
            ignore=shutil.ignore_patterns("_manifests"),
        )
        return dst

    def write_report(self) -> dict[str, object]:
        self.report_dir.mkdir(parents=True, exist_ok=True)
        report = {
            "label": self.args.label,
            "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "data_root": self.args.data_root,
            "profile": self.args.profile,
            "scenario": self.args.scenario,
            "base_url": self.base_url,
            "passed": all(self.checks.values()),
            "checks": self.checks,
            "metrics": self.metrics,
            "jobs": self.jobs,
            "db_path": str(self.db_path),
            "log_dir": str(self.log_dir),
            "run_dir": str(self.run_dir),
        }
        json_path = self.report_dir / f"scan_mutation_stress_{self.args.label}.json"
        md_path = self.report_dir / f"scan_mutation_stress_{self.args.label}.md"
        json_path.write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        md_path.write_text(render_markdown(report), encoding="utf-8")
        return {
            "passed": report["passed"],
            "report_json": str(json_path),
            "report_md": str(md_path),
        }


def hardlink_or_copy(src: str, dst: str) -> None:
    try:
        os.link(src, dst)
    except OSError:
        shutil.copy2(src, dst)


def list_photos(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and path.suffix.lower() in {".jpg", ".jpeg"}
    )


def render_markdown(report: dict[str, object]) -> str:
    metrics = report["metrics"]
    checks = report["checks"]
    jobs = report["jobs"]
    lines = [
        f"# Scan Mutation Stress Report: {report['label']}",
        "",
        f"- Passed: `{report['passed']}`",
        f"- Profile: `{report['profile']}`",
        f"- Scenario: `{report['scenario']}`",
        f"- Data root: `{report['data_root']}`",
        f"- Baseline photos: `{metrics['baseline_count']}`",
        f"- Deleted files: `{metrics['delete_count']}`",
        f"- Appended files: `{metrics['append_count']}`",
        f"- DB path: `{report['db_path']}`",
        f"- Log dir: `{report['log_dir']}`",
        "",
        "## Correctness Checks",
        "",
        "| Check | Result |",
        "| --- | --- |",
    ]
    for key, value in checks.items():
        lines.append(f"| `{key}` | `{value}` |")

    lines.extend(
        [
            "",
            "## Source Counts",
            "",
            "| Phase | API Total | Active | Deleted | FTS Rows |",
            "| --- | ---: | ---: | ---: | ---: |",
        ]
    )
    for phase in ["initial", "after_delete", "after_append"]:
        item = metrics[phase]
        db = item["db"]
        lines.append(
            f"| `{phase}` | {item['api_total']} | {db['active_photos']} | "
            f"{db['deleted_photos']} | {db['fts_rows']} |"
        )
    retry = metrics["retry"]
    retry_db = retry["db"]
    lines.append(
        f"| `retry_after_fix` | {retry['api_total']} | {retry_db['active_photos']} | "
        f"{retry_db['deleted_photos']} | {retry_db['fts_rows']} |"
    )
    in_scan = metrics["in_scan_mutation"]
    for phase, item in [
        ("in_scan_baseline", in_scan["baseline"]),
        ("in_scan_after_mutation_job", in_scan["after_mutation_job"]),
        ("in_scan_converged", in_scan["converged"]),
    ]:
        db = item["db"]
        lines.append(
            f"| `{phase}` | {item['api_total']} | {db['active_photos']} | "
            f"{db['deleted_photos']} | {db['fts_rows']} |"
        )

    lines.extend(
        [
            "",
            "## In-Scan Mutation",
            "",
            f"- Pre-mutation observed job status: `{in_scan['pre_mutation_status']}`",
            f"- Baseline photos: `{in_scan['baseline_count']}`",
            f"- Deleted while scan was active: `{in_scan['delete_count']}`",
            f"- Appended while scan was active: `{in_scan['append_count']}`",
            f"- Expected after convergence: `{in_scan['expected_after_converge']}`",
            "",
            "## Jobs",
            "",
            "| Label | Status | Total | Processed | New | Updated | Failed |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    for label, job in jobs.items():
        lines.append(
            f"| `{label}` | `{job['status']}` | {job.get('total_count')} | "
            f"{job.get('processed_count')} | {job.get('new_count')} | "
            f"{job.get('updated_count')} | {job.get('failed_count')} |"
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    runner = StressRunner(args)
    try:
        result = runner.run()
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0 if result["passed"] else 1
    finally:
        runner.cleanup()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        os.kill(os.getpid(), signal.SIGTERM)
