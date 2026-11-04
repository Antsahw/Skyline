"""The N (default 60) most-downloaded Fabric 26.3 mods on Modrinth with their required
dependencies: plan.json and jars/ in SURVEY_DIR (default /tmp/hz-survey). Then
inspect.py plan.json jars, survey.py, analyze.py (docs/java-bridge-survey.md)."""
import json, os, sys, urllib.request, urllib.parse, time
ua={"User-Agent":"iaxivers/horizon-bridge-survey"}
def get(u):
    for i in range(4):
        try: return json.load(urllib.request.urlopen(urllib.request.Request(u,headers=ua),timeout=60))
        except Exception as e: err=e; time.sleep(2)
    raise err
q=urllib.parse.urlencode({"game_versions":'["26.3"]',"loaders":'["fabric"]'})
vcache={}
def ver(pid):
    if pid not in vcache:
        vs=get(f"https://api.modrinth.com/v2/project/{pid}/version?{q}")
        rel=[v for v in vs if v["version_type"]=="release"] or vs
        vcache[pid]=rel[0] if rel else None
    return vcache[pid]
os.chdir(os.environ.get("SURVEY_DIR", "/tmp/hz-survey")) if os.makedirs(os.environ.get("SURVEY_DIR", "/tmp/hz-survey"), exist_ok=True) is None else None
N=int(sys.argv[1]) if len(sys.argv) > 1 else 60
facets=json.dumps([["project_type:mod"],["versions:26.3"],["categories:fabric"]])
hits=[]
for off in (0,100):
    hits+=get("https://api.modrinth.com/v2/search?"+urllib.parse.urlencode({"facets":facets,"index":"downloads","limit":100,"offset":off}))["hits"]
picked=[]; skipped=[]
for h in hits:
    if len(picked)>=N: break
    v=ver(h["project_id"])
    if v: picked.append((h,v))
    else: skipped.append(h["slug"])
print("skipped (no 26.3 fabric file):",skipped)
proj={}
def pname(pid):
    if pid not in proj: proj[pid]=get(f"https://api.modrinth.com/v2/project/{pid}")
    return proj[pid]
out=[]
for h,v in picked:
    deps={}; stack=[v]; missing=[]
    while stack:
        cur=stack.pop()
        for d in cur["dependencies"]:
            if d["dependency_type"]!="required" or not d.get("project_id"): continue
            pid=d["project_id"]
            if pid in deps or pid==h["project_id"]: continue
            dv=ver(pid)
            if not dv: missing.append(pname(pid)["slug"]); deps[pid]=None; continue
            deps[pid]=dv; stack.append(dv)
    files=[]
    for vv in [v]+[d for d in deps.values() if d]:
        f=next((f for f in vv["files"] if f["primary"]),vv["files"][0])
        files.append({"pid":vv["project_id"],"slug":pname(vv["project_id"])["slug"],"version":vv["version_number"],"file":f["filename"],"url":f["url"],"sha1":f["hashes"]["sha1"]})
    out.append({"slug":h["slug"],"title":h["title"],"downloads":h["downloads"],"client":h["client_side"],"server":h["server_side"],"files":files,"missing":missing})
    print(h["slug"],[f["slug"] for f in files[1:]],missing and "MISSING "+str(missing) or "")
json.dump(out,open("plan.json","w"),indent=1)
import hashlib
os.makedirs("jars",exist_ok=True)
for f in {f["file"]:f for m in out for f in m["files"]}.values():
    if not os.path.exists("jars/"+f["file"]):
        d=urllib.request.urlopen(urllib.request.Request(f["url"],headers=ua),timeout=120).read()
        assert hashlib.sha1(d).hexdigest()==f["sha1"],f["file"]
        open("jars/"+f["file"],"wb").write(d)
