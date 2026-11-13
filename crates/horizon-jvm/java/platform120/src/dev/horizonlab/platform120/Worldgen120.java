package dev.horizonlab.platform120;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.serialization.JsonOps;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.RegistryOps;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.levelgen.GenerationStep;
import net.minecraft.world.level.levelgen.feature.ConfiguredFeature;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/**
 * The 1.20.1 mods' world generation as Horizon's generator takes it (phase 6).
 *
 * <p>A Fabric mod puts its ores and plants into vanilla's biomes in code, through Fabric API's
 * {@code BiomeModifications} (selectors are Java predicates; the features are data). A real server
 * applies those as it starts ({@code BiomeModificationImpl.finalizeWorldGen}, from Fabric's own
 * MinecraftServer mixin, which the mirror server's skipped constructor never reaches). Here they
 * are applied to the runtime's worldgen registries (vanilla's and the mods' data packs, loaded as
 * a server loads them), and each biome's feature lists are compared with what they were: what
 * the mods added and took away, step by step, is what Horizon needs, written as NeoForge's biome
 * modifiers would say it. The features themselves go with it, encoded by their own codecs, for
 * those a mod registers in code rather than in its data.
 */
final class Worldgen120 {
    private Worldgen120() {}

    private static String changes;

    /**
     * {@code {"added": {biome: {step: [feature...]}}, "removed": {...}, "placed": {feature: json},
     * "configured": {feature: json}}}, made once (the modifications apply once to a registry).
     */
    static synchronized String biomeChanges() {
        if (changes == null) changes = compute();
        return changes;
    }

    private static String compute() {
        RegistryAccess registries = Mirror120.registries();
        Registry<Biome> biomes = registries.registryOrThrow(Registries.BIOME);
        Map<ResourceKey<Biome>, List<Set<String>>> before = snapshot(biomes);
        Map<ResourceKey<Biome>, Map<String, JsonObject>> spawnsBefore = spawns(biomes);
        try {
            net.fabricmc.fabric.impl.biome.modification.BiomeModificationImpl.INSTANCE.finalizeWorldGen(registries);
        } catch (RuntimeException | LinkageError e) {
            Mirror120.log("the mods' biome modifications failed: " + e);
        }
        Map<ResourceKey<Biome>, List<Set<String>>> after = snapshot(biomes);
        // The mobs added to each biome's spawns (BiomeModifications.addSpawn).
        JsonObject spawnsAdded = new JsonObject();
        Map<ResourceKey<Biome>, Map<String, JsonObject>> spawnsAfter = spawns(biomes);
        for (var entry : new TreeMap<String, ResourceKey<Biome>>(keys(spawnsAfter)).entrySet()) {
            Map<String, JsonObject> was = spawnsBefore.getOrDefault(entry.getValue(), Map.of());
            JsonArray plus = new JsonArray();
            for (var spawner : spawnsAfter.get(entry.getValue()).entrySet()) {
                if (!was.containsKey(spawner.getKey())) plus.add(spawner.getValue());
            }
            if (!plus.isEmpty()) spawnsAdded.add(entry.getKey(), plus);
        }
        JsonObject added = new JsonObject();
        JsonObject removed = new JsonObject();
        Set<String> features = new LinkedHashSet<>();
        for (var entry : new TreeMap<String, ResourceKey<Biome>>(keys(after)).entrySet()) {
            var key = entry.getValue();
            List<Set<String>> was = before.getOrDefault(key, List.of());
            List<Set<String>> now = after.get(key);
            JsonObject add = new JsonObject();
            JsonObject take = new JsonObject();
            int steps = Math.max(was.size(), now.size());
            for (int step = 0; step < steps; step++) {
                Set<String> a = step < was.size() ? was.get(step) : Set.of();
                Set<String> b = step < now.size() ? now.get(step) : Set.of();
                JsonArray plus = new JsonArray();
                for (String f : b) if (!a.contains(f)) { plus.add(f); features.add(f); }
                JsonArray minus = new JsonArray();
                for (String f : a) if (!b.contains(f)) minus.add(f);
                String name = stepName(step);
                if (name == null) continue;
                if (!plus.isEmpty()) add.add(name, plus);
                if (!minus.isEmpty()) take.add(name, minus);
            }
            if (add.size() > 0) added.add(entry.getKey(), add);
            if (take.size() > 0) removed.add(entry.getKey(), take);
        }
        var ops = RegistryOps.create(JsonOps.INSTANCE, registries);
        Registry<PlacedFeature> placed = registries.registryOrThrow(Registries.PLACED_FEATURE);
        JsonObject placedJson = new JsonObject();
        JsonObject configuredJson = new JsonObject();
        for (String id : features) {
            PlacedFeature feature = placed.get(new net.minecraft.resources.ResourceLocation(id));
            if (feature == null) continue;
            try {
                placedJson.add(id, PlacedFeature.DIRECT_CODEC.encodeStart(ops, feature).getOrThrow(false, e -> {}));
            } catch (RuntimeException e) {
                Mirror120.log("feature " + id + " can't be written as data: " + e);
                continue;
            }
            Holder<ConfiguredFeature<?, ?>> inner = feature.feature();
            var innerKey = inner.unwrapKey();
            if (innerKey.isPresent()) {
                String innerId = innerKey.get().location().toString();
                try {
                    configuredJson.add(innerId, ConfiguredFeature.DIRECT_CODEC.encodeStart(ops, inner.value()).getOrThrow(false, e -> {}));
                } catch (RuntimeException e) {
                    Mirror120.log("feature " + innerId + " can't be written as data: " + e);
                }
            }
        }
        JsonObject out = new JsonObject();
        out.add("added", added);
        out.add("removed", removed);
        out.add("placed", placedJson);
        out.add("configured", configuredJson);
        out.add("spawns", spawnsAdded);
        return out.toString();
    }

    /** The mods' placed features: those of a namespace not vanilla's. */
    static String[] modFeatures() {
        Registry<PlacedFeature> placed = Mirror120.registries().registryOrThrow(Registries.PLACED_FEATURE);
        return placed.keySet().stream()
                .filter(id -> !"minecraft".equals(id.getNamespace()))
                .map(Object::toString)
                .sorted()
                .toArray(String[]::new);
    }

    private static Generator120 generator;

    /** HORIZON_120_TRACE: each feature placed is said. */
    private static final boolean TRACE = System.getenv("HORIZON_120_TRACE") != null;

    /**
     * Places a mod's placed feature where vanilla's {@code ChunkGenerator.applyBiomeDecoration}
     * would: its random seeded as there (the chunk's decoration seed, then the feature's index
     * and step), then {@code PlacedFeature.placeWithBiomeCheck} on the level, whose blocks,
     * heights and biomes are the chunks Horizon is decorating.
     */
    static boolean place(net.minecraft.server.level.ServerLevel level, String id, long worldSeed,
            int chunkX, int chunkZ, int index, int step, net.minecraft.core.BlockPos origin) {
        biomeChanges();
        RegistryAccess registries = Mirror120.registries();
        PlacedFeature feature = registries.registryOrThrow(Registries.PLACED_FEATURE)
                .get(new net.minecraft.resources.ResourceLocation(id));
        if (feature == null) return false;
        if (generator == null) {
            generator = new Generator120(registries.registryOrThrow(Registries.BIOME)
                    .getHolderOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS));
        }
        var random = new net.minecraft.world.level.levelgen.WorldgenRandom(
                new net.minecraft.world.level.levelgen.XoroshiroRandomSource(net.minecraft.world.level.levelgen.RandomSupport.generateUniqueSeed()));
        long seed = random.setDecorationSeed(worldSeed, chunkX << 4, chunkZ << 4);
        random.setFeatureSeed(seed, index, step);
        try {
            boolean placed = feature.placeWithBiomeCheck(level, generator, random, origin);
            if (placed && TRACE) Mirror120.log("placed " + id + " in chunk " + chunkX + " " + chunkZ);
            return placed;
        } catch (RuntimeException | LinkageError e) {
            Mirror120.log("feature " + id + " failed at " + origin.toShortString() + ": " + e);
            return false;
        }
    }

    private static Map<String, ResourceKey<Biome>> keys(Map<ResourceKey<Biome>, ?> map) {
        Map<String, ResourceKey<Biome>> out = new TreeMap<>();
        for (var key : map.keySet()) out.put(key.location().toString(), key);
        return out;
    }

    /** Each biome's placed features, step by step, by id (features with no id are left out). */
    private static Map<ResourceKey<Biome>, List<Set<String>>> snapshot(Registry<Biome> biomes) {
        Map<ResourceKey<Biome>, List<Set<String>>> out = new java.util.HashMap<>();
        for (var entry : biomes.entrySet()) {
            List<Set<String>> steps = new ArrayList<>();
            for (var step : entry.getValue().getGenerationSettings().features()) {
                Set<String> ids = new LinkedHashSet<>();
                for (Holder<PlacedFeature> holder : step) {
                    holder.unwrapKey().ifPresent(k -> ids.add(k.location().toString()));
                }
                steps.add(ids);
            }
            out.put(entry.getKey(), steps);
        }
        return out;
    }

    /** Each biome's spawners, keyed by all they are (category, type, weight, counts). */
    private static Map<ResourceKey<Biome>, Map<String, JsonObject>> spawns(Registry<Biome> biomes) {
        Map<ResourceKey<Biome>, Map<String, JsonObject>> out = new java.util.HashMap<>();
        for (var entry : biomes.entrySet()) {
            Map<String, JsonObject> spawners = new java.util.LinkedHashMap<>();
            var settings = entry.getValue().getMobSettings();
            for (var category : net.minecraft.world.entity.MobCategory.values()) {
                for (var data : settings.getMobs(category).unwrap()) {
                    JsonObject one = new JsonObject();
                    one.addProperty("type", net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(data.type).toString());
                    one.addProperty("category", category.getName());
                    one.addProperty("weight", data.getWeight().asInt());
                    one.addProperty("minCount", data.minCount);
                    one.addProperty("maxCount", data.maxCount);
                    spawners.put(one.toString(), one);
                }
            }
            out.put(entry.getKey(), spawners);
        }
        return out;
    }

    private static String stepName(int step) {
        GenerationStep.Decoration[] all = GenerationStep.Decoration.values();
        return step < all.length ? all[step].name().toLowerCase(Locale.ROOT) : null;
    }
}
