import json, zipfile, sys, re, os
plan=json.load(open(sys.argv[1])); jd=sys.argv[2]
def info(path):
    z=zipfile.ZipFile(path); names=z.namelist()
    fm={}
    try: fm=json.loads(z.read("fabric.mod.json").decode("utf8","replace"),strict=False)
    except Exception as e: fm={"_err":str(e)}
    mid=fm.get("id"); ep=fm.get("entrypoints",{})
    # nested jars
    nested=[n for n in names if n.startswith("META-INF/jars/") and n.endswith(".jar")]
    bs=sorted({n.split("/")[3][:-5] for n in names if re.match(rf"assets/{re.escape(mid or '')}/blockstates/[^/]+\.json$",n)}) if mid else []
    ents=[]
    for n in names:
        if mid and re.match(rf"assets/{re.escape(mid)}/lang/en_us\.json$",n):
            try:
                lang=json.loads(z.read(n).decode("utf8","replace"),strict=False)
                ents=sorted({k.split(".")[2] for k in lang if k.startswith(f"entity.{mid}.") and k.count(".")==2})
            except: pass
    return {"id":mid,"name":fm.get("name"),"env":fm.get("environment","*"),"modmenu":"modmenu" in ep,
            "entrypoints":sorted(ep),"mixins":len(fm.get("mixins",[])),"blocks":bs,"entities":ents,"nested":len(nested),"fm_err":fm.get("_err")}
for m in plan:
    i=info(os.path.join(jd,m["files"][0]["file"])); m["info"]=i
    print(f'{m["slug"]:28} id={i["id"]} env={i["env"]} menu={i["modmenu"]} blocks={len(i["blocks"])} ents={len(i["entities"])} ep={i["entrypoints"]}')
json.dump(plan,open(sys.argv[1],"w"),indent=1)
