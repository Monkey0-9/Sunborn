import json
import sys
import urllib.request

run_id = sys.argv[1] if len(sys.argv) > 1 else "latest"
if run_id == "latest":
    url = "https://api.github.com/repos/Monkey0-9/Sunborn/actions/runs?per_page=1"
else:
    url = f"https://api.github.com/repos/Monkey0-9/Sunborn/actions/runs/{run_id}/jobs"

req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
try:
    with urllib.request.urlopen(req) as resp:
        data = json.loads(resp.read().decode("utf-8"))
        jobs = data.get("jobs", [])
        if not jobs and "workflow_runs" in data and data["workflow_runs"]:
            latest_run = data["workflow_runs"][0]
            print(f"Latest Run ID: {latest_run.get('id')} — Status: {latest_run.get('status')} / {latest_run.get('conclusion')}")
        else:
            for j in jobs:
                print(f"  {j['name']}: {j['status']} / {j['conclusion']}")
except Exception as e:
    print(f"Failed to query CI jobs: {e}")
