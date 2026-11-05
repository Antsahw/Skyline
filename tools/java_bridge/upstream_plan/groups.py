import re, os, sys, collections
P = "/home/claude/wt-boss/crates/horizon-jvm/java/platform/dev/horizonlab/platform"
G = {
 "J2": "HorizonLog HorizonServices HorizonFailures HorizonProfile HorizonJson PlatformPlugin ModsPlugin",
 "J4": "HorizonServer EntityAccess LevelAccess MinecraftServerAccess PlayerAdvancementsAccess PlayerListAccess ServerCommonListenerAccess ServerLevelAccess",
 "J5": "MinecraftServerMixin ServerLevelMixin LevelMixin ServerChunkCacheMixin ChunkMapMixin PlayerListMixin GameModeMixin DedicatedServerMixin IntegratedServerMixin ServerPlayerMixin ServerPlayerHurtMixin EventLoopMixin TimerCallbackMixin GameListenerMixin RespawnDataMixin PlayerLookupMixin EntitySelectorMixin RiderAuthorityMixin",
 "J6": "HorizonBlockEntities HorizonTicks HorizonLight HorizonChunks HorizonChunkLoads HorizonTickets HorizonItems HorizonHoppers HorizonFluids HorizonNeighbours HorizonVanillaActions HorizonSavedData HorizonFunctions HorizonCombat HorizonData HorizonDataFixer FunctionsMixin CombatCaptureMixin FallCaptureMixin EntityFluidInteractionMixin BlockScanMixin PathNavigationRegionMixin WorldGenRegionMixin",
 "J7": "HorizonHooks HorizonMixinTable HorizonMixinInit",
 "J8": "HorizonContent",
 "J9": "HorizonClient",
 "J10": "HorizonClientState HorizonClientBlockEntities HorizonClientCommands ClientLevelMixin CameraMixin MouseHandlerMixin ClientChunkCacheMixin ClientPlayerMixin ClientChatMixin ChatComponentMixin LocalPlayerMessagesMixin MinecraftCameraMixin MinecraftProfileMixin GameModeClientMixin SkinDownloaderMixin SkinManagerMixin TextInputManagerMixin EditBoxFocusMixin LevelExtractorMixin",
 "J11": "HorizonLoop HorizonPacketTap HorizonConfiguration ConnectionMixin MirrorConnectionMixin MirrorPacketContextMixin MirrorUpdatesMixin PacketListenerMixin PlayNetworkingMixin ClientPlayNetworkingMixin ClientCommonPacketListenerMixin TapConnectionMixin",
 "J12": "HorizonGraphics HorizonGraphicsWrappers HorizonScreenEvents HorizonTextures HorizonSprites HorizonItemModels HorizonLazyModels HorizonClientConfig HorizonModMenuApi HorizonModsPage HorizonOverlayScreen HorizonTextFields HorizonMonitors GuiGraphicsMixin GuiMixin HudMixin ScreenMixin RecipeBookComponentMixin DynamicTextureMixin TextureAtlasMixin TextureManagerMixin ModelBakeryMixin ModelManagerMixin ItemModelResolverMixin ItemStackRenderStateMixin SlotDisplayContextMixin MaterialSystemMixin",
 "J13": "HorizonEntities HorizonEntityRender HorizonSpawner HorizonPackets HorizonParticles HorizonSounds SoundManagerMixin",
 "J14": "HorizonWorldgen HorizonNoiseExport HorizonSurface HorizonBiomes HorizonBiomeMemo HorizonTints BiomeSourceMixin ChunkGeneratorMixin NoiseFillMixin",
 "J15": "HorizonGpu",
 "J16": "HorizonDeviceMods HorizonDeviceClient",
}
cls2g = {}
for g, s in G.items():
    for c in s.split(): cls2g[c] = g
files = {}
for root, _, fs in os.walk(P):
    for f in fs:
        if f.endswith(".java"): files[f[:-5]] = os.path.join(root, f)
missing = [c for c in files if c not in cls2g]
print("UNMAPPED:", missing)
lines = collections.Counter()
for c, p in files.items(): lines[cls2g.get(c,"?")] += sum(1 for _ in open(p))
print("LINES:", dict(sorted(lines.items(), key=lambda x:int(x[0][1:]) if x[0][1:].isdigit() else 0)))
edges = collections.defaultdict(lambda: collections.Counter())
names = set(files)
for c, p in files.items():
    src = open(p).read()
    src = re.sub(r'//.*', '', src)
    src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)
    g = cls2g.get(c)
    for n in names:
        if n != c and re.search(r'\b'+n+r'\b', src):
            h = cls2g.get(n)
            if h != g: edges[g][(h, c, n)] += 1
order = ["J2","J4","J5","J6","J7","J8","J9","J10","J11","J12","J13","J14","J15","J16"]
idx = {g:i for i,g in enumerate(order)}
back = collections.defaultdict(list)
for g, cs in edges.items():
    for (h, c, n) in cs:
        if idx[h] > idx[g]: back[(g,h)].append(f"{c}->{n}")
for (g,h), l in sorted(back.items(), key=lambda x:(idx[x[0][0]], idx[x[0][1]])):
    print(f"BACKREF {g} -> {h}: {len(l)}: {', '.join(sorted(l)[:8])}{' ...' if len(l)>8 else ''}")

# class-level graph and SCCs
adj = collections.defaultdict(set)
for c, p in files.items():
    src = open(p).read(); src = re.sub(r'//.*', '', src); src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)
    for n in names:
        if n != c and re.search(r'\b'+n+r'\b', src): adj[c].add(n)
sys.setrecursionlimit(10000)
index={}; low={}; st=[]; on=set(); sccs=[]; i=[0]
def sc(v):
    index[v]=low[v]=i[0]; i[0]+=1; st.append(v); on.add(v)
    for w in adj[v]:
        if w not in index: sc(w); low[v]=min(low[v],low[w])
        elif w in on: low[v]=min(low[v],index[w])
    if low[v]==index[v]:
        comp=[]
        while True:
            w=st.pop(); on.discard(w); comp.append(w)
            if w==v: break
        sccs.append(comp)
for v in files:
    if v not in index: sc(v)
for comp in sccs:
    if len(comp)>1:
        gs=collections.Counter(cls2g[c] for c in comp)
        print("SCC", len(comp), "classes, groups:", dict(gs))
print("GROUP EDGES (excluding sources HorizonServer, HorizonClient):")
ge = collections.defaultdict(collections.Counter)
for c in files:
    if c in ("HorizonServer","HorizonClient"): continue
    for n in adj[c]:
        if cls2g[n]!=cls2g[c]: ge[cls2g[c]][cls2g[n]]+=1
for g in order:
    print(g, dict(ge[g]))
print("DETAIL")
for a,b in [("J7","J6"),("J13","J12"),("J15","J12"),("J12","J15"),("J10","J13"),("J13","J10"),("J12","J13"),("J16","J12"),("J16","J11"),("J11","J16"),("J13","J9"),("J12","J14"),("J14","J6"),("J13","J6")]:
    l=[f"{c}->{n}" for c in files if cls2g[c]==a and c not in ("HorizonServer","HorizonClient") for n in adj[c] if cls2g[n]==b]
    print(a,b,l)
