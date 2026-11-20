import json
import os
import subprocess
import sys
import urllib.request

token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
if not token:
    try:
        p = subprocess.Popen(
            ["git", "credential", "fill"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        stdout, _ = p.communicate("protocol=https\nhost=github.com\n")
        for line in stdout.splitlines():
            if line.startswith("password="):
                token = line.split("=", 1)[1]
                break
    except Exception:
        pass

headers = {"User-Agent": "Mozilla/5.0"}
if token:
    headers["Authorization"] = f"token {token}"

run_arg = sys.argv[1] if len(sys.argv) > 1 else None

if not run_arg:
    url = "https://api.github.com/repos/Monkey0-9/Sunborn/actions/runs?per_page=6"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            for r in data.get("workflow_runs", []):
                sha = r.get("head_commit", {}).get("id", "")[:7]
                print(f"Run #{r.get('id')} [{r.get('name')}]: commit {sha} - status: {r.get('status')} / conclusion: {r.get('conclusion')}")
    except Exception as e:
        print(f"Failed to query CI runs: {e}")
else:
    url = f"https://api.github.com/repos/Monkey0-9/Sunborn/actions/runs/{run_arg}/jobs"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            jobs = data.get("jobs", [])
            for j in jobs:
                print(f"  {j['name']}: {j['status']} / {j['conclusion']}")
                for step in j.get('steps', []):
                    if step.get('status') == 'in_progress' or step.get('conclusion') == 'failure':
                        print(f"      -> Step: {step.get('name')} [{step.get('status')} / {step.get('conclusion')}]")
    except Exception as e:
        print(f"Failed to query CI jobs: {e}")


