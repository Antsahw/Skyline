import re, subprocess, collections, sys
sys.argv=["x"]
exec(open("/tmp/plan/hunks.py").read().split("for path in sys.argv[1:]:")[0])
G=lambda *a: subprocess.run(["git","-C","/home/claude/horizon"]+list(a),capture_output=True,text=True).stdout
ns=[l.split("\t") for l in G("diff","--numstat",MB,FB,"--",".",":!crates/horizon-jvm",":!tools/java_bridge",":!docs/java-bridge*").splitlines()]
st={l.split("\t")[1]:l.split("\t")[0] for l in G("diff","--name-status",MB,FB,"--",".",":!crates/horizon-jvm",":!tools/java_bridge",":!docs/java-bridge*").splitlines()}
C="crates/horizon-client/src/"; RR="crates/horizon-client-render/src/"; W="crates/horizon-world/src/"
def newfile(p):
    rules=[
     ("FORK", r"java-mods-release\.yml|^README\.md|^AGENTS\.md|java-mods-keys|java_release\.rs|research-lock"),
     ("A1", r"horizon-protocol/"),("A2", r"heap_trim|horizon-threads/src/lib|mem_log|level/data/streaming|state_memory_probe|texture_memory_probe"),
     ("A3", r"baked_quad|models/cube"),("A4", r"pack_templates|structure/packs\.rs|datapack/reader"),
     ("A5", r"noise_seed|density/graph"),("A6", r"horizon-mods/"),("A7", r"block_collision|collision/mod_templates|collision\.rs|mod_block_collision"),
     ("A8", r"mod_content"),("A9a", r"lenient_json|jar\.rs|legacy_lang|texture_namespaces|fabric_ids|pipeline\.rs"),("A9b", r"carry/declare|carry\.rs"),("A9c", r"oracle|invisible"),
     ("A10", r"horizon-assets/"),("A12", r"replay"),("A13", r"streaming/busy"),
     ("A14", r"integrated/thread_end|mounts/jump|machines/dropper|mods/cache|native-mods\.md|state_mesh_scan"),
     ("B1", r"level/(mod|fluids|update|tests|caller_precipitation)"),("B2", r"noise_only|start/unsupported|start/tests/unsupported|vanillagen/java|generator_structures|start\.rs"),
     ("B3", r"mesher/java|assets/java"),("B4", r"hud/java|particles/java|biome/java|item_render/java|block_revision"),("B5", r"datapack/java|streaming/java"),
     ("C0", r"java_mods\.rs$|java_mods/off|horizon-client/Cargo\.toml"),
     ("C1", r"menu/java|machines/java|levels/java"),("C2", r"advancements/java|combat/java|landing/java|trial_spawner/java|server_travel/java"),
     ("C3", r"options_ui/java"),("C4", r"names/java|mounts/java"),("C5", r"save/java|create_world/java|dedicated/java|initial_spawn/java|chat/java|lan/data/java|world_loading/java"),
     ("C6", r"lan/java|lan_actions/java|lan_crafting/java|hub/java|guest/java|containers/java|conn/java|packet_tap"),("C7", r"remote/java"),
     ("G0", r"java_mods/bridge|java_profile"),("G1", r"java_server\.rs"),("G2", r"java_screen\.rs"),("G3", r"java_entities|creatures/java"),("G4", r"java_worldgen|noise_pack_probe"),("G7", r"java_device"),
     ("J1", r"^Cargo\.(toml|lock)|horizon-prepare|check_public_tree|test_check_no_fabric|\.gitignore"),
     ("L", r"^launcher/|launcher-commands"),("CI", r"game-build\.yml"),("E2E", r"lan_e2e"),("A12", r"en_us\.json"),
    ]
    for pr,rx in rules:
        if re.search(rx,p): return pr
    return "??"
OV={ # file: {hunk: PR}
 RR+"assets.rs":{1:"B3",5:"B3"}, RR+"biome.rs":{1:"B4",2:"B4"}, RR+"chunk.rs":{1:"B4",4:"B4",5:"B4",6:"B4"},
 RR+"datapack.rs":{1:"B5",2:"B5",3:"B5",4:"B5",6:"B5",5:"A4",7:"A4"}, RR+"gfx.rs":{1:"G7",2:"G7",3:"G7"},
 RR+"hud.rs":{1:"B4",2:"B4",3:"B4",4:"B4"}, RR+"mesher.rs":{1:"B3",2:"B3",3:"B3",4:"B3",5:"B3",7:"B3",8:"B3"},
 RR+"mesher/fluid.rs":{1:"B3",2:"B3",3:"B3"}, RR+"mod_list.rs":{1:"B4",2:"B4"}, RR+"options_ui.rs":{i:"C3" for i in range(1,8)},
 RR+"particles.rs":{1:"B4"}, RR+"streaming.rs":{1:"A13",2:"B5",3:"B5",4:"A13",5:"A13",6:"A13",7:"A13",8:"A13",9:"A13",10:"A13"},
 RR+"world.rs":{1:"C1",2:"C1"}, C+"bin/horizon-ctl/launch.rs":{2:"A14"}, C+"game/commands.rs":{1:"A11"},
 C+"game/lan_actions.rs":{**{i:"C6" for i in range(1,33)},21:"G7"}, C+"game/lan_crafting.rs":{4:"C6"}, C+"game/lan_vehicles.rs":{1:"C4"},
 C+"game/stations.rs":{1:"C6"}, C+"lan/containers.rs":{3:"C6",4:"C6"}, C+"mobs.rs":{1:"C4"},
 C+"mod_content.rs":{i:"A8" for i in range(1,27)}, C+"mod_content_tests.rs":{i:"A8" for i in range(1,8)},
 C+"title/create_world.rs":{7:"C5"}, C+"vanillagen.rs":{1:"B2",2:"B2",3:"B2",4:"B2",5:"B2",6:"B2",8:"B2"},
 C+"item_render.rs":{1:"B4",2:"B4"}, C+"game/creatures.rs":{2:"G3",3:"G3",4:"G3",5:"G3",6:"G3"},
 C+"game/remote.rs":{2:"G7",3:"G7",7:"G4",8:"G7"}, C+"game/remote_vehicles.rs":{1:"G3"}, C+"game/agent.rs":{6:"G3"},
 C+"game/initial_spawn.rs":{3:"C5"}, C+"game/mods.rs":{2:"G0",3:"G0"}, C+"entry.rs":{1:"G0"},
 C+"game/world_loading.rs":{1:"C5"},
 W+"level/data.rs":{1:"A2",2:"A2"}, W+"level/mod.rs":{1:"B1"},
 W+"worldgen.rs":{1:"A5",2:"A5"}, W+"worldgen/density.rs":{1:"A5"},
 W+"worldgen/generator.rs":{**{i:"B2" for i in range(1,14)},2:"A4",3:"A5"},
 W+"worldgen/overrides.rs":{i:"A4" for i in range(1,5)}, W+"worldgen/structure.rs":{i:"A4" for i in range(1,5)},
 W+"block_collision.rs":{i:"A7" for i in range(1,7)}, W+"collision.rs":{1:"A7",2:"A7",3:"A7"},
 C+"game.rs":{1:"C0",6:"C0",70:"G0"},
}
tot=collections.Counter(); files=collections.defaultdict(set)
for a,d,p in ns:
    a=int(a) if a!="-" else 0; d=int(d) if d!="-" else 0
    if st.get(p)=="A" or not (p.endswith(".rs") and p.startswith("crates/") and not p.startswith(("crates/horizon-assets","crates/horizon-core","crates/horizon-protocol","crates/horizon-threads","crates/horizon-mods","crates/horizon-port-pack","crates/horizon-prepare"))) or p.endswith("/examples/"):
        pr=newfile(p); tot[pr]+=a+d; files[pr].add(p.split("/src/")[-1] if "/src/" in p else p); continue
    hs=hunks(diff(p)); ov=OV.get(p,{})
    for i,h in enumerate(hs,1):
        pr=ov.get(i) or classify(h)
        if pr=="C0" and "game/" in p and p.endswith(".rs") and "mod java;" in "\n".join(h): pr="(modline)"
        n=sum(1 for l in h[1:] if l[:1] in "+-")
        tot[pr]+=n; files[pr].add(p.split("/src/")[-1]+f"#{i}")
for k in sorted(tot): print(k, tot[k], " ".join(sorted(files[k]))[:900])
