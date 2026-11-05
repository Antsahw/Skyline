import re, subprocess, sys, collections
MB="41d5649ed5b676065c2227a597e6e9ea9f370ebb"; FB="f90ab51b2c4538679e72a4f6165e3c617c5b0e1c"
def diff(path):
    return subprocess.run(["git","-C","/home/claude/horizon","diff","-U3",MB,FB,"--",path],capture_output=True,text=True).stdout
def hunks(text):
    out=[]; cur=None
    for l in text.splitlines():
        if l.startswith("@@"):
            cur=[l]; out.append(cur)
        elif cur is not None: cur.append(l)
    return out
R=[ # (regex on added/removed lines, PR)
 (r"replay|Replay", "A12"),
 (r"mem_mark|mem_log|trim_heap|behaviour_json|from_file", "A2"),
 (r"BakedQuad|force_translucent|by_layer|baked block models", "A3"),
 (r"Dim::Extra|\.vanilla\(\)|coordinate_scale|extra_level|Dim::all|Dim::known|client_level|dimension_type_of|DIMENSION_TYPE|ExtraLevel|Extra\(u8\)|fn known|fn all\(\)|level_dim|d\.id\(\)|dim\.id\(\)|self\.dim\.id\(\)|extra_level_stem", "A11"),
 (r"drawn_at|level_load\.is_active|CTL_LAUNCH|locate_structure|structure\.invalid|\"biome\"|may_fly\"|\"passenger\"|near_loading|thread_end|loop returned|player spawn: |charge_jump|drop_into|entity_container_at|serve_local", "A14"),
 (r"held_join|hold_join", "C6"),
 (r"JavaMod\b|Screen::JavaMod|java_screen|draw_java_hud|draw_java_overlay|java_bind_|JavaModSettings|open_java_mod_settings", "C3"),
 (r"java_entity|java_hand|riding_java|java_mount|java_jump_bar|java_name_tags|java_dismount|java_remote_ride|java_fresh_chunk|java_neighbours_seen|java_mob_hits|java_projectile|java_mob_attack", "C4"),
 (r"mod_preset|presets|PRESET_KEY|spawn_origin|java_spawn|level_named|mod_structure_packs|java::streaming|add_mod_biomes|add_mod_structures|push_mod_biomes|java::mod_preset", "C5"),
 (r"remote_java|java_new_server|remote_use_java|remote_attack_java|java_device|tick_device_client|java_mod_fluids|java_mod_item_models", "C7"),
 (r"mod_window|java_actor|set_actor|java_guest|JumpsRefused|refused\.|java_routed_falls|java_menu|java_entity_attack|java_entity_interact|java_use_block|java_use_item|guest_java_command|java_attack_block|tapped|\.tap\(|send_login_finished|send_tapped|lan_java_crafted|ModAbilities|mod_abilities|tab_name|java_window_state|speeds\(\)|invulnerable\(\)|may_fly\(\)|open_unplaced|open_container_at|DEDICATED_ONLY|INTEGRATED_ONLY|packet_tap|java_player_chat|java_event_except|java_world_closing|look_at_entity|java::wide_frame|wide", "C6"),
 (r"java_award|add_effect_asking_mods|java_add_effect|java_trial|transform_bad_omen|route_fall|Routed|java_hurt|java_player_teleport|java_portal|java_level_teleport|local_java_command|java_fall|java_jumped|landing", "C2"),
 (r"foreign_fluid|liquid_interaction|observe_neighbours|java_random_tick|java_precipitation|java_holds_item|java_break|java_after_break|java_block_event|tick_java_server|java_crafted|crafted_takes|CraftedTake|save_java_mods|apply_java_level_writes|java_tickets|set_ticketed|parked_items|java_eject|java_suck|java_item_storage|java_drop|java_random_ticks|JavaServerState|java_mods::JavaGame|java: ", "C1"),
 (r"mod java;|mod java_|horizon_jvm|start_java_mods|java-mods|start_early|java_creative|java_content|horizon_cache|mod cache", "C0"),
]
def classify(h):
    body="\n".join(l for l in h[1:] if l[:1] in "+-")
    for rx,pr in R:
        if re.search(rx, body): return pr
    return "??"
for path in sys.argv[1:]:
    hs=hunks(diff(path)); c=collections.defaultdict(list)
    for i,h in enumerate(hs,1):
        k=classify(h); c[k].append(i)
        if k=="??" or "-v" in sys.argv[0:0]:
            first=next((l for l in h[1:] if l[:1] in "+-" and l[1:].strip()),"")
            print(f"  ?? {path.split('/')[-1]}#{i}: {first[:100]}")
    print(path, len(hs), {k: v for k,v in sorted(c.items())})
