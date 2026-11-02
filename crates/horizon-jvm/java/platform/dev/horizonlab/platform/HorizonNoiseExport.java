package dev.horizonlab.platform;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.mojang.serialization.JsonOps;
import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryOps;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.densityfunction.DensityFunction;
import net.minecraft.world.level.levelgen.densityfunction.DensityFunctions;
import net.minecraft.world.level.levelgen.densityfunction.DfRewriteRule;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * The land's shape of a dimension the mods change (Tectonic's density functions), written as
 * a data pack Horizon's own noise reads for far terrain: the dimension's noise settings, every
 * density function and noise, as the loaded registries have them (the mods' configs applied).
 * Density function types of a mod are written as vanilla ones where they are one in disguise
 * (Tectonic's config constants, clamps and noises); any other makes the export fail, and far
 * terrain keeps Horizon's vanilla noise.
 */
public final class HorizonNoiseExport {
    private HorizonNoiseExport() {}

    /** A density function type no vanilla one stands for. */
    private static final class Unsupported extends RuntimeException {
        Unsupported(String type) {
            super(type, null, false, false);
        }
    }

    /**
     * Writes the pack into {@code dir} (pack.mcmeta and data/): the noise settings of the
     * dimension's generator under its own id. Null when written, else why not.
     */
    public static String export(RegistryAccess registries, ChunkGenerator generator, Path dir, long seed,
            net.minecraft.world.level.levelgen.RandomState real) {
        if (!(generator instanceof NoiseBasedChunkGenerator noise)) return "not a noise generator";
        var settingsHolder = noise.generatorSettings();
        var settingsKey = settingsHolder.unwrapKey().orElse(null);
        if (settingsKey == null) return "its noise settings have no id";
        RegistryOps<JsonElement> ops = registries.createSerializationContext(JsonOps.INSTANCE);
        var gson = new GsonBuilder().create();
        int written = 0;
        try {
            Files.createDirectories(dir);
            Files.writeString(dir.resolve("pack.mcmeta"),
                    "{\"pack\":{\"description\":\"The Java mods' noise, for Horizon's far terrain\",\"pack_format\":94}}");
            // The noise settings with the router's mods' nodes made vanilla; then every
            // density function it names, and theirs (vanilla() keeps references and records
            // them; a mod's node may hide some from rewriteChildren: Tectonic's clamp's
            // bounds), and the noises all of them name. Only those: other mods' entries may be
            // of types or values with no vanilla form (Enderscape's End).
            NoiseGeneratorSettings settings = settingsHolder.value();
            var router = settings.noiseRouter();
            REFS.get().clear();
            var plainRouter = new net.minecraft.world.level.levelgen.NoiseRouter(
                    vanilla(router.temperature()), vanilla(router.vegetation()), vanilla(router.continents()),
                    vanilla(router.erosion()), vanilla(router.depth()), vanilla(router.ridges()),
                    vanilla(router.chunkSurfaceLevel()), vanilla(router.finalDensity()));
            var plainSettings = new NoiseGeneratorSettings(settings.noiseSettings(), settings.defaultBlock(),
                    settings.defaultFluid(), plainRouter, settings.materialRule(), settings.spawnTarget(),
                    settings.seaLevel(), settings.disableMobGeneration(), settings.aquifers(),
                    settings.useLegacyRandomSource(), settings.debugFunctions());
            var settingsJson = NoiseGeneratorSettings.DIRECT_CODEC.encodeStart(ops, plainSettings)
                    .getOrThrow(IllegalStateException::new);
            // Far terrain's tops come from the mods' rules through Java (lodTops) or are
            // vanilla's: the rule here stays vanilla's (mods merge theirs into a form of their
            // own: Biolith's "base" and "sources").
            settingsJson.getAsJsonObject().addProperty("material_rule", settingsKey.identifier().toString());
            var functions = registries.lookupOrThrow(Registries.DENSITY_FUNCTION);
            var functionIds = new java.util.HashSet<String>();
            for (var ref : functions.listElements().toList()) functionIds.add(ref.key().identifier().toString());
            var noises = registries.lookupOrThrow(Registries.NOISE);
            var noiseIds = new java.util.HashSet<String>();
            for (var ref : noises.listElements().toList()) noiseIds.add(ref.key().identifier().toString());
            var named = new java.util.TreeSet<String>();
            var queue = new java.util.ArrayDeque<String>();
            for (Identifier id : REFS.get()) queue.add(id.toString());
            names(settingsJson, functionIds, queue);
            names(settingsJson, noiseIds, named);
            write(dir, settingsKey.identifier(), "noise_settings", gson.toJson(settingsJson));
            written++;
            var done = new java.util.HashSet<String>();
            while (!queue.isEmpty()) {
                String id = queue.poll();
                if (!done.add(id)) continue;
                Identifier identifier = Identifier.parse(id);
                var value = functions.get(net.minecraft.resources.ResourceKey.create(Registries.DENSITY_FUNCTION, identifier))
                        .orElseThrow(() -> new IllegalStateException("no density function " + id)).value();
                REFS.get().clear();
                DensityFunction plain = vanilla(value);
                for (Identifier ref : REFS.get()) queue.add(ref.toString());
                var json = DensityFunction.CODEC.encodeStart(ops, plain).getOrThrow(IllegalStateException::new);
                names(json, functionIds, queue);
                names(json, noiseIds, named);
                write(dir, identifier, "density_function", gson.toJson(json));
                written++;
            }
            for (var ref : noises.listElements().toList()) {
                if (!named.contains(ref.key().identifier().toString())) continue;
                var json = NormalNoise.DIRECT_CODEC.encodeStart(ops, ref.value()).getOrThrow(IllegalStateException::new);
                String seedId = seedOf(real, ref.key(), ref.value());
                if (seedId == null) return "the noise " + ref.key().identifier() + " is seeded some way of a mod's";
                if (!seedId.equals(ref.key().identifier().toString())) json.getAsJsonObject().addProperty("horizon_seed", seedId);
                write(dir, ref.key().identifier(), "noise", gson.toJson(json));
                written++;
            }
            if (System.getenv("HORIZON_JAVA_NOISE_EXPORT_CHECK") != null) check(registries, settings, plainSettings, seed, real);
            Natives.log("Java mods: far terrain's noise exported (" + written + " files, " + settingsKey.identifier() + ")");
            return null;
        } catch (Unsupported u) {
            return "the density function type " + u.getMessage() + " has no vanilla equivalent";
        } catch (Throwable t) {
            return t.toString();
        }
    }

    private static void write(Path dir, Identifier id, String kind, String json) throws java.io.IOException {
        Path file = dir.resolve("data").resolve(id.getNamespace()).resolve("worldgen").resolve(kind)
                .resolve(id.getPath() + ".json");
        Files.createDirectories(file.getParent());
        Files.writeString(file, json);
    }

    /** HORIZON_JAVA_NOISE_EXPORT_CHECK: the final density of the mods' router against the written one's. */
    private static void check(RegistryAccess registries, NoiseGeneratorSettings original, NoiseGeneratorSettings plain, long seed,
            net.minecraft.world.level.levelgen.RandomState real) {
        var noises = registries.lookupOrThrow(Registries.NOISE);
        // The level's own (mods may set it up: Tectonic's configs), against one made here.
        var a = real;
        Natives.log("Java mods: far terrain's noise check, seed " + seed);
        var b = net.minecraft.world.level.levelgen.RandomState.create(noises, seed, plain);
        var fa = a.getSampler(original.noiseRouter().finalDensity());
        var fb = b.getSampler(plain.noiseRouter().finalDensity());
        var none = net.minecraft.world.level.levelgen.densityfunction.SamplerContext.EMPTY_UNCACHED;
        float worst = 0;
        String at = "";
        var random = new java.util.Random(1);
        for (int i = 0; i < 200; i++) {
            int x = random.nextInt(4000) - 2000, y = random.nextInt(200) - 20, z = random.nextInt(4000) - 2000;
            float va = fa.sampleValue(none, x, y, z), vb = fb.sampleValue(none, x, y, z);
            if (Math.abs(va - vb) > worst) {
                worst = Math.abs(va - vb);
                at = x + " " + y + " " + z + ": " + va + " vs " + vb;
            }
        }
        Natives.log("Java mods: far terrain's noise check: worst difference " + worst + (at.isEmpty() ? "" : " at " + at));
        int[][] points = {{0, 64, 0}, {500, 64, 500}, {-800, 70, 300}, {1200, 100, -900}, {300, 40, -1500}};
        for (int[] p : points) {
            StringBuilder line = new StringBuilder("Java mods: far terrain's noise at " + p[0] + " " + p[1] + " " + p[2] + ":");
            for (var debug : original.debugFunctions().functions()) {
                line.append(' ').append(debug.label()).append('=')
                        .append(a.getSampler(debug.function()).sampleValue(none, p[0], p[1], p[2]));
            }
            String probe = System.getenv("HORIZON_JAVA_NOISE_PROBE");
            if (probe != null) {
                var functions = registries.lookupOrThrow(Registries.DENSITY_FUNCTION);
                for (String id : probe.split(",")) {
                    if (id.startsWith("noise:")) {
                        var key = net.minecraft.resources.ResourceKey.create(Registries.NOISE, Identifier.parse(id.substring(6)));
                        line.append(' ').append(id).append('=').append(a.getOrCreateNoise(key).get(p[0], 0, p[2]));
                        continue;
                    }
                    var f = functions.get(net.minecraft.resources.ResourceKey.create(Registries.DENSITY_FUNCTION, Identifier.parse(id)));
                    if (f.isEmpty()) continue;
                    line.append(' ').append(id).append('=').append(a.getSampler(f.get().value()).sampleValue(none, p[0], p[1], p[2]));
                }
            }
            Natives.log(line.toString());
        }
    }

    /**
     * The identifier the level's random state seeds {@code key}'s noise from: its own, or a
     * mod's other one (Tectonic's NoisesMixin seeds tectonic:parameter/x as minecraft:x),
     * found by comparing the noise made each way. Null if none matches.
     */
    private static String seedOf(net.minecraft.world.level.levelgen.RandomState state,
            net.minecraft.resources.ResourceKey<NormalNoise> key, NormalNoise noise) {
        try {
            var field = net.minecraft.world.level.levelgen.RandomState.class.getDeclaredField("random");
            field.setAccessible(true);
            var factory = (net.minecraft.world.level.levelgen.PositionalRandomFactory) field.get(state);
            var real = state.getOrCreateNoise(key);
            String path = key.identifier().getPath();
            String[] candidates = {key.identifier().toString(),
                    "minecraft:" + path.substring(path.lastIndexOf('/') + 1),
                    "minecraft:" + path.substring(path.indexOf('/') + 1)};
            for (String candidate : candidates) {
                var made = noise.create(factory.fromHashOf(Identifier.parse(candidate)));
                boolean same = true;
                for (int[] p : new int[][] {{0, 0}, {317, -1201}, {-2048, 640}}) {
                    if (made.get(p[0], 0, p[1]) != real.get(p[0], 0, p[1])) {
                        same = false;
                        break;
                    }
                }
                if (same) return candidate;
            }
            return null;
        } catch (ReflectiveOperationException e) {
            return key.identifier().toString();
        }
    }

    /** The registry density functions vanilla() kept as references, this thread's. */
    private static final ThreadLocal<java.util.Set<Identifier>> REFS = ThreadLocal.withInitial(java.util.HashSet::new);

    /** The strings in {@code json} that are ids in {@code ids}. */
    private static void names(JsonElement json, java.util.Set<String> ids, java.util.Collection<String> out) {
        if (json.isJsonPrimitive() && json.getAsJsonPrimitive().isString()) {
            String v = json.getAsString();
            if (ids.contains(v)) out.add(v);
        } else if (json.isJsonArray()) {
            for (var e : json.getAsJsonArray()) names(e, ids, out);
        } else if (json.isJsonObject()) {
            for (var e : json.getAsJsonObject().entrySet()) names(e.getValue(), ids, out);
        }
    }

    /** The rule that makes each node vanilla, children first; references stay references. */
    private static final DfRewriteRule VANILLA = new DfRewriteRule() {
        @Override
        public DensityFunction rewrite(DensityFunction f) {
            return vanilla(f);
        }
    };

    static DensityFunction vanilla(DensityFunction f) {
        if (f instanceof DensityFunctions.HolderHolder holder) {
            // A registry entry stays a reference (it is written on its own); an inline one is
            // made vanilla in place.
            var key = holder.function().unwrapKey();
            if (key.isPresent()) {
                REFS.get().add(key.get().identifier());
                return f;
            }
            return vanilla(holder.function().value());
        }
        var key = BuiltInRegistries.DENSITY_FUNCTION_TYPE.getKey(f.codec());
        String type = key == null ? f.getClass().getName() : key.toString();
        if (key != null && key.getNamespace().equals("minecraft")) return f.rewriteChildren(VANILLA);
        // A mod's type: a constant if its range is one value.
        var range = f.range();
        if (!range.isNaI() && range.min() == range.max()) return DensityFunctions.constant(range.min());
        try {
            switch (type) {
                case "tectonic:config_clamp": {
                    // clamp(input, min, max) = max(min(input, max), min)
                    DensityFunction input = vanilla((DensityFunction) call(f, "input"));
                    DensityFunction min = vanilla((DensityFunction) call(f, "min"));
                    DensityFunction max = vanilla((DensityFunction) call(f, "max"));
                    return DensityFunctions.max(DensityFunctions.min(input, max), min);
                }
                case "tectonic:config_noise": {
                    // noise(x * scale + shiftX, 0, z * scale + shiftZ) * multiplier + offset
                    @SuppressWarnings("unchecked")
                    Holder<NormalNoise> noise = (Holder<NormalNoise>) call(f, "noise");
                    DensityFunction sx = vanilla((DensityFunction) call(f, "shiftX"));
                    DensityFunction sz = vanilla((DensityFunction) call(f, "shiftZ"));
                    float scale = (float) call(f, "scale"), mul = (float) call(f, "multiplier"), off = (float) call(f, "offset");
                    return DensityFunctions.add(
                            DensityFunctions.mul(DensityFunctions.shiftedNoise2d(sx, sz, scale, noise), DensityFunctions.constant(mul)),
                            DensityFunctions.constant(off));
                }
                default:
                    throw new Unsupported(type);
            }
        } catch (ReflectiveOperationException e) {
            throw new Unsupported(type + " (" + e + ")");
        }
    }

    private static Object call(Object o, String accessor) throws ReflectiveOperationException {
        Method m = o.getClass().getMethod(accessor);
        m.setAccessible(true);
        return m.invoke(o);
    }
}
