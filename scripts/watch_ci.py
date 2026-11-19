import urllib.request, json

url = "https://api.github.com/repos/Monkey0-9/Sunborn/actions/runs/44359456166/jobs"
req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
with urllib.request.urlopen(req) as resp:
    data = json.loads(resp.read().decode("utf-8"))
    for j in data.get("jobs", []):
        print(f"  {j['name']}: {j['status']} / {j['conclusion']}")
