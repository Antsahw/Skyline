package dev.horizonlab.bridge.r120;

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import dev.horizonlab.bridge.remote.Host;
import dev.horizonlab.bridge.remote.Peer;
import dev.horizonlab.bridge.remote.RemoteRuntime;

/**
 * Fabric 1.20.1 with unmodified mods, in a JVM of its own (docs/java-bridge-120.md, phase 1).
 * Started by horizon-jvm-remote's boot120.rs through {@code remote.Child}; standalone with
 * {@link #main} (tools/java_bridge/run120.sh).
 *
 * <p>The class path is a Fabric launcher profile's for 1.20.1 (fetch120.py's launch.json):
 * Fabric Loader and its libraries, the intermediary mappings, Mojang's libraries, Mojang's
 * client jar. So Fabric's own Minecraft provider runs as it does under a launcher: it finds the
 * game, remaps it from Mojang's obfuscated names to intermediary (cached in
 * {@code <game>/.fabric/remappedJars}), finds the mods and applies their Mixins. Only the start
 * differs: no game window and no game loop. The registries are bootstrapped as vanilla's own
 * start does, then each mod's {@code main} and {@code client} entrypoints run one by one, so
 * a mod that fails is recorded (the report's {@code entrypoints}) instead of stopping the rest.
 *
 * <p>Fabric is reached by reflection, so this class compiles and loads with JDK types only
 * (like Boot112), and touches only loader entry points that have been stable for years
 * (Knot's constructor and {@code init}, {@code prepareModInit}, the public FabricLoader API):
 * a pack that needs an older loader can still run (the plan's "older Fabric Loader" section).
 *
 * <p>Calls from Horizon: {@code start} (boots, returns the report), {@code report}, {@code ping}.
 */
public final class Boot120 implements RemoteRuntime {
    /** What to start: parsed from {@code --key value} arguments. */
    static final class Options {
        Path gameJar, mappings, mods, gameDir;
        List<Path> addMods = new ArrayList<>();
        String side = "client";

        static Options parse(String[] args) {
            Options o = new Options();
            for (int i = 0; i + 1 < args.length; i += 2) {
                String v = args[i + 1];
                switch (args[i]) {
                    case "--game-jar" -> o.gameJar = Path.of(v);
                    case "--mappings" -> o.mappings = Path.of(v);
                    case "--mods" -> o.mods = Path.of(v);
                    case "--game-dir" -> o.gameDir = Path.of(v);
                    case "--side" -> o.side = v;
                    case "--add-mods" -> {
                        for (String p : v.split(File.pathSeparator)) if (!p.isEmpty()) o.addMods.add(Path.of(p));
                    }
                    default -> throw new IllegalArgumentException("unknown option " + args[i]);
                }
            }
            if (o.gameJar == null || o.mappings == null || o.gameDir == null) {
                throw new IllegalArgumentException("--game-jar, --mappings and --game-dir are needed");
            }
            if (o.mods == null) o.mods = o.gameDir.resolve("mods");
            if (!o.side.equals("client") && !o.side.equals("server")) throw new IllegalArgumentException("--side client|server");
            return o;
        }
    }

    private Options options;
    private ClassLoader game;
    private Names120 names;
    private final Map<String, Object> timings = new LinkedHashMap<>();
    /** mod id -> entrypoint key -> "ok" or the failure. */
    private final Map<String, Map<String, String>> entrypoints = new LinkedHashMap<>();
    private final List<String> problems = new ArrayList<>();
    /** Mods whose declared Fabric Loader range was overridden (Overrides120). */
    private Map<String, Map<String, String>> overrides = Map.of();
    private String report;

    @Override
    public void init(Peer peer, String[] args) {
        options = Options.parse(args);
    }

    @Override
    public Object[] call(Peer peer, String method, Object[] args) throws Exception {
        switch (method) {
            case "start":
                if (report == null) report = boot();
                return new Object[] {report};
            case "report":
                if (game == null) throw new IllegalStateException("not started");
                return new Object[] {report = new Report120(this).write()};
            case "facts":
                // The mods' content for Horizon (platform120's Content120): the facts file
                // horizon-port-pack's carry-over reads; returns the mods' namespaces.
                if (game == null) throw new IllegalStateException("not started");
                return new Object[] {facts((String) args[0])};
            case "useBlock":
                // A right-click of Horizon's player on a block (platform120's Mirror120.useBlock).
                return mirror("useBlock", args);
            case "tickBlockEntities":
                // One tick of the mods' block entities Horizon knows of (Mirror120).
                return mirror("tickBlockEntities", args);
            case "screenFrame":
                // A frame of the screen of a player's open menu (Mirror120, Screens120).
                return mirror("screenFrame", args);
            case "neighbours":
                // Horizon's neighbour updates beside the mods' blocks (Mirror120.neighbours).
                return mirror(method, args);
            case "drawnByCode":
            case "renderBlock":
                // Blocks whose look is the mods' code, drawn as their models draw them (Models120).
                if (game == null) throw new IllegalStateException("not started");
                return mirror(method, args);
            case "summon":
            case "tickEntities":
            case "renderEntities":
            case "renderBlockEntities":
            case "entityAction":
            case "saveEntities":
            case "loadEntities":
            case "clearEntities":
            case "modEntityTypes":
            case "modBlockEntityBlocks":
            case "configMods":
            case "openConfig":
            case "hud":
            case "commandNames":
            case "commandTree":
            case "command":
            case "spawnNatural":
            case "spawnGenerated":
            case "animateTicks":
                // The mods' own entities (Entities120).
                if (game == null) throw new IllegalStateException("not started");
                return mirror(method, args);
            case "modFeatures":
            case "placeFeature":
                // The mods' own features, placed in the chunks Horizon generates (Worldgen120).
                if (game == null) throw new IllegalStateException("not started");
                return mirror(method, args);
            case "biomeChanges":
                // What the mods' Fabric biome modifications add to and take from vanilla's
                // biomes, as data (Mirror120, Worldgen120).
                if (game == null) throw new IllegalStateException("not started");
                return mirror(method, args);
            case "keyProbe":
            case "modKeys":
            case "keyPress":
                // The mods' key bindings, and a player's keys for them (Mirror120, Keys120).
                return mirror(method, args);
            case "playerLeft":
                // A player of Horizon's left the game (Mirror120.playerLeft).
                return mirror("playerLeft", args);
            case "useItem":
                // A right-click with a mod's item at nothing (Mirror120.useItem).
                return mirror("useItem", args);
            case "packetProbe":
                // A round trip through the mods' packets, for the tests (Mirror120, Packets120).
                return mirror("packetProbe", args);
            case "serverTick":
                // One server tick: Fabric API's tick and lifecycle events (Mirror120, Events120).
                return mirror("serverTick", args);
            case "breakBlock":
                // A player of Horizon's breaks a block (Mirror120.breakBlock).
                return mirror("breakBlock", args);
            case "hopperInsert":
            case "hopperExtract":
                // Horizon's hoppers and droppers at a mod's block (Mirror120, Hoppers120).
                return mirror(method, args);
            case "screenInput":
                // A player's click, key or close on their open screen (Mirror120, Screens120).
                return mirror("screenInput", args);
            case "screenProbe":
                // What a screen's drawing records (Screens120): the recording end to end.
                return mirror("screenProbe", args);
            case "blockShapes":
            case "collisionShapes":
            case "blockSignals":
            case "signalSources":
                // What the mods' blocks are at these positions: their outline, what is solid of
                // them, the redstone they give (Mirror120).
                if (game == null) throw new IllegalStateException("not started");
                return mirror(method, args);
            case "ping":
                return new Object[] {"pong"};
            default:
                throw new UnsupportedOperationException(method);
        }
    }

    /**
     * Writes the content facts to {@code out} with the platform's Content120 (the platform jar,
     * build120.sh's, among the mods); returns the mods' namespaces, comma separated.
     */
    String facts(String out) throws Exception {
        Thread thread = Thread.currentThread();
        ClassLoader before = thread.getContextClassLoader();
        thread.setContextClassLoader(game);
        try {
            Class<?> content = Class.forName("dev.horizonlab.platform120.Content120", true, game);
            return (String) content.getMethod("exportFacts", String.class).invoke(null, out);
        } catch (ClassNotFoundException e) {
            throw new IllegalStateException("the platform jar isn't loaded (tools/java_bridge/build120.sh, then --add-mods it)");
        } catch (java.lang.reflect.InvocationTargetException e) {
            throw e.getCause() instanceof Exception x ? x : e;
        } finally {
            thread.setContextClassLoader(before);
        }
    }

    private boolean mirrorHooked;

    /**
     * Calls platform120's Mirror120 (the 1.20.1 world as Horizon's) on the game's class loader,
     * its hooks into Horizon set first: the arguments as Mirror120's method takes them, boxed.
     */
    Object[] mirror(String method, Object[] args) throws Exception {
        Thread thread = Thread.currentThread();
        ClassLoader before = thread.getContextClassLoader();
        thread.setContextClassLoader(game);
        try {
            Class<?> mirror = Class.forName("dev.horizonlab.platform120.Mirror120", true, game);
            if (!mirrorHooked) {
                java.util.function.BiFunction<String, Object[], Object[]> call = Host::call;
                java.util.function.BiConsumer<String, Object[]> event = Host::event;
                mirror.getField("call").set(null, call);
                mirror.getField("event").set(null, event);
                mirrorHooked = true;
            }
            for (var m : mirror.getMethods()) {
                if (m.getName().equals(method) && m.getParameterCount() == args.length) {
                    Object[] converted = new Object[args.length];
                    Class<?>[] types = m.getParameterTypes();
                    for (int i = 0; i < args.length; i++) converted[i] = convert(args[i], types[i]);
                    return (Object[]) m.invoke(null, converted);
                }
            }
            throw new UnsupportedOperationException("Mirror120." + method + "/" + args.length);
        } catch (ClassNotFoundException e) {
            throw new IllegalStateException("the platform jar isn't loaded (tools/java_bridge/build120.sh, then --add-mods it)");
        } catch (java.lang.reflect.InvocationTargetException e) {
            // The mods' code failed in Horizon's world: where, for the log (the error itself
            // goes back as the call's answer).
            var trace = new java.io.StringWriter();
            e.getCause().printStackTrace(new java.io.PrintWriter(trace));
            trace.toString().lines().limit(25).forEach(l -> Host.log("Mirror120." + method + ": " + l));
            throw e.getCause() instanceof Exception x ? x : e;
        } finally {
            thread.setContextClassLoader(before);
        }
    }

    /** A wire value as a parameter: the wire's ints for booleans and doubles too. */
    private static Object convert(Object v, Class<?> type) {
        if (type == boolean.class) return v instanceof Boolean b ? b : ((Number) v).intValue() != 0;
        if (type == double.class) return ((Number) v).doubleValue();
        if (type == float.class) return ((Number) v).floatValue();
        if (type == int.class) return ((Number) v).intValue();
        if (type == long.class) return ((Number) v).longValue();
        return v;
    }

    Options options() {
        return options;
    }

    ClassLoader game() {
        return game;
    }

    Names120 names() {
        return names;
    }

    Map<String, Object> timings() {
        return timings;
    }

    Map<String, Map<String, String>> entrypoints() {
        return entrypoints;
    }

    Map<String, Map<String, String>> overrides() {
        return overrides;
    }

    List<String> problems() {
        return problems;
    }

    private static long ms(long since) {
        return (System.nanoTime() - since) / 1_000_000;
    }

    /** Boots Fabric 1.20.1 and the mods; returns the report. */
    String boot() throws Exception {
        long started = System.nanoTime();
        Files.createDirectories(options.gameDir);
        Files.createDirectories(options.mods);
        logConfig();
        // Mods held back only by their declared loader range get Fabric's own dependency
        // override, before Fabric Loader resolves them (Overrides120).
        long t0 = System.nanoTime();
        try {
            overrides = Overrides120.write(options.gameDir, options.mods, options.addMods);
            for (Map.Entry<String, Map<String, String>> e : overrides.entrySet()) {
                Host.log("mod " + e.getKey() + " declares Fabric Loader " + e.getValue().get("fabricloader")
                        + ": overridden (" + e.getValue().get("by") + ")");
            }
        } catch (Exception e) {
            problems.add("dependency overrides: " + root(e));
        }
        timings.put("overrides_ms", ms(t0));
        System.setProperty("fabric.gameJarPath", options.gameJar.toAbsolutePath().toString());
        System.setProperty("fabric.modsFolder", options.mods.toAbsolutePath().toString());
        if (!options.addMods.isEmpty()) {
            List<String> paths = new ArrayList<>();
            for (Path p : options.addMods) paths.add(p.toAbsolutePath().toString());
            System.setProperty("fabric.addMods", String.join(File.pathSeparator, paths));
        }

        // Knot: Fabric's class loader, its Minecraft provider (remaps the game), mod discovery,
        // Mixin. The arguments are a launcher's (the provider reads --gameDir).
        long t = System.nanoTime();
        Class<?> envType = Class.forName("net.fabricmc.api.EnvType");
        @SuppressWarnings({"unchecked", "rawtypes"})
        Object env = Enum.valueOf((Class) envType, options.side.toUpperCase(java.util.Locale.ROOT));
        Class<?> knotClass = Class.forName("net.fabricmc.loader.impl.launch.knot.Knot");
        Object knot = knotClass.getConstructor(envType).newInstance(env);
        String[] launchArgs = {"--gameDir", options.gameDir.toAbsolutePath().toString(), "--accessToken", "0", "--version", "1.20.1"};
        game = (ClassLoader) knotClass.getMethod("init", String[].class).invoke(knot, (Object) launchArgs);
        timings.put("knot_ms", ms(t));

        t = System.nanoTime();
        names = Names120.load(options.mappings, resolver());
        timings.put("names_ms", ms(t));

        // What vanilla's start runs before Fabric's entrypoints: the version, the built-in
        // registries and their check (Main for a server, Minecraft's constructor for a client).
        t = System.nanoTime();
        Class<?> constants = names.cls("net.minecraft.SharedConstants", game);
        names.callStatic(constants, "tryDetectVersion");
        Class<?> bootstrap = names.cls("net.minecraft.server.Bootstrap", game);
        names.callStatic(bootstrap, "bootStrap");
        timings.put("bootstrap_ms", ms(t));

        Object loader = Class.forName("net.fabricmc.loader.impl.FabricLoaderImpl").getField("INSTANCE").get(null);
        Class.forName("net.fabricmc.loader.impl.FabricLoaderImpl").getMethod("prepareModInit", Path.class, Object.class)
                .invoke(loader, options.gameDir.toAbsolutePath(), null);

        if (options.side.equals("client")) {
            // A client's Minecraft exists before Fabric's entrypoints run (its constructor
            // starts them): mods' main code may reach it too.
            t = System.nanoTime();
            String failed = Headless120.make(names, game, options.gameDir.toAbsolutePath());
            if (failed != null) problems.add("the headless client: " + failed);
            timings.put("headless_ms", ms(t));
        }

        t = System.nanoTime();
        runEntrypoints("main", "net.fabricmc.api.ModInitializer", "onInitialize");
        if (options.side.equals("client")) {
            runEntrypoints("client", "net.fabricmc.api.ClientModInitializer", "onInitializeClient");
            String failed = Headless120.options(names, game, options.gameDir.toAbsolutePath());
            if (failed != null) problems.add("the headless client's options: " + failed);
        } else {
            runEntrypoints("server", "net.fabricmc.api.DedicatedServerModInitializer", "onInitializeServer");
        }
        timings.put("entrypoints_ms", ms(t));

        // Fabric API holds vanilla's freeze back until the mods have registered; its own
        // MainMixin / MinecraftClient hook then freezes. Neither runs here, so the same call.
        t = System.nanoTime();
        try {
            Class<?> registries = names.cls("net.minecraft.core.registries.BuiltInRegistries", game);
            names.callStatic(registries, "bootStrap");
        } catch (ReflectiveOperationException e) {
            problems.add("freezing the registries: " + root(e));
        }
        timings.put("freeze_ms", ms(t));
        timings.put("start_ms", ms(started));
        Runtime rt = Runtime.getRuntime();
        timings.put("heap_mb", (rt.totalMemory() - rt.freeMemory()) >> 20);
        Host.log("Fabric 1.20.1 ready in " + timings.get("start_ms") + " ms: " + entrypoints.size() + " mods with entrypoints, "
                + problems.size() + " problems");
        return new Report120(this).write();
    }

    /** Fabric's MappingResolver from official names to the run-time namespace. */
    private static Names120.Translator resolver() throws ReflectiveOperationException {
        Class<?> fabric = Class.forName("net.fabricmc.loader.api.FabricLoader");
        Object instance = fabric.getMethod("getInstance").invoke(null);
        Object resolver = fabric.getMethod("getMappingResolver").invoke(instance);
        Class<?> api = Class.forName("net.fabricmc.loader.api.MappingResolver");
        Method mapClass = api.getMethod("mapClassName", String.class, String.class);
        Method mapMethod = api.getMethod("mapMethodName", String.class, String.class, String.class, String.class);
        Method mapField = api.getMethod("mapFieldName", String.class, String.class, String.class, String.class);
        return new Names120.Translator() {
            private String invoke(Method m, Object... args) {
                try {
                    return (String) m.invoke(resolver, args);
                } catch (ReflectiveOperationException e) {
                    throw new IllegalStateException(e);
                }
            }

            @Override
            public String cls(String official) {
                return invoke(mapClass, "official", official.replace('/', '.'));
            }

            @Override
            public String method(String owner, String name, String descriptor) {
                return invoke(mapMethod, "official", owner.replace('/', '.'), name, descriptor);
            }

            @Override
            public String field(String owner, String name, String descriptor) {
                return invoke(mapField, "official", owner.replace('/', '.'), name, descriptor);
            }
        };
    }

    /**
     * Runs one kind of entrypoint, mod by mod. Fabric's own invokeEntrypoints stops at the first
     * failure of a {@code main} entrypoint; here each failure is recorded and the rest run.
     */
    private void runEntrypoints(String key, String type, String methodName) throws ReflectiveOperationException {
        Class<?> api = Class.forName("net.fabricmc.loader.api.FabricLoader");
        Object fabric = api.getMethod("getInstance").invoke(null);
        Class<?> typeClass = Class.forName(type);
        Method invoke = typeClass.getMethod(methodName);
        List<?> containers = (List<?>) api.getMethod("getEntrypointContainers", String.class, Class.class).invoke(fabric, key, typeClass);
        int failed = 0;
        for (Object container : containers) {
            String id = (String) Api.call(Api.call(Api.call(container, "getProvider"), "getMetadata"), "getId");
            Map<String, String> mine = entrypoints.computeIfAbsent(id, k -> new LinkedHashMap<>());
            try {
                Object entrypoint = Api.call(container, "getEntrypoint");
                invoke.invoke(entrypoint);
                mine.merge(key, "ok", (a, b) -> a);
            } catch (Throwable e) {
                failed++;
                String why = root(e);
                mine.put(key, why);
                Host.log("mod " + id + ": its " + key + " entrypoint failed: " + why);
                Throwable cause = cause(e);
                StackTraceElement[] frames = cause.getStackTrace();
                for (int f = 0; f < Math.min(frames.length, 8); f++) Host.log("    at " + frames[f]);
            }
        }
        Host.log(containers.size() + " " + key + " entrypoints, " + failed + " failed");
    }

    private static Throwable cause(Throwable e) {
        Throwable root = e;
        while (root.getCause() != null && root.getCause() != root) root = root.getCause();
        return root;
    }

    static String root(Throwable e) {
        if (e instanceof InvocationTargetException && e.getCause() != null) e = e.getCause();
        Throwable root = cause(e);
        return root == e ? String.valueOf(e) : e + ", caused by " + root;
    }

    /**
     * Log4j's configuration for the game's and mods' log lines: to stdout, which Horizon turns
     * into its own log lines. A launcher passes Mojang's file; without one log4j prints only
     * errors.
     */
    private void logConfig() throws java.io.IOException {
        if (System.getProperty("log4j.configurationFile") != null) return;
        Path file = options.gameDir.resolve("horizon-log4j2.xml");
        Files.writeString(file, """
                <?xml version="1.0" encoding="UTF-8"?>
                <Configuration status="WARN">
                  <Appenders>
                    <Console name="out" target="SYSTEM_OUT">
                      <PatternLayout pattern="[%d{HH:mm:ss}] [%t/%level] [%logger{1}]: %msg%n"/>
                    </Console>
                  </Appenders>
                  <Loggers>
                    <Root level="info"><AppenderRef ref="out"/></Root>
                  </Loggers>
                </Configuration>
                """);
        System.setProperty("log4j.configurationFile", file.toAbsolutePath().toString());
    }

    /** Standalone: {@code Boot120 [--report FILE] [--facts FILE] --game-jar ... --mappings ... --game-dir ...}. */
    public static void main(String[] args) throws Exception {
        String out = null, factsOut = null;
        List<String> rest = new ArrayList<>();
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--report") && i + 1 < args.length) out = args[++i];
            else if (args[i].equals("--facts") && i + 1 < args.length) factsOut = args[++i];
            else rest.add(args[i]);
        }
        Boot120 boot = new Boot120();
        boot.options = Options.parse(rest.toArray(new String[0]));
        String report = boot.boot();
        if (out != null) Files.writeString(Path.of(out), report);
        Host.log("report: " + report.length() + " chars" + (out != null ? " -> " + out : ""));
        if (factsOut != null) Host.log("facts for " + boot.facts(factsOut) + " -> " + factsOut);
        System.exit(0);
    }
}
