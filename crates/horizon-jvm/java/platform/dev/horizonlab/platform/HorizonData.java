package dev.horizonlab.platform;

import com.mojang.serialization.Lifecycle;
import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.commands.Commands;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.MappedRegistry;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ReloadableServerResources;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.CloseableResourceManager;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.util.Util;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.DataPackConfig;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.levelgen.presets.WorldPresets;

/**
 * The server's data, loaded as vanilla's game-test server loads it: the vanilla data pack
 * (and those Fabric mods add), giving the full registries with their tags bound (biomes,
 * dimension types, damage types...) and the reloadable resources (recipes, loot, advancements,
 * functions, and the command dispatcher, where Fabric API's command callback registers mods'
 * commands). Loaded once, after the mods' entrypoints, before Horizon's first tick.
 */
public final class HorizonData {
    private HorizonData() {}

    record Loaded(CloseableResourceManager resources, ReloadableServerResources reloadable,
                  LayeredRegistryAccess<RegistryLayer> registries,
                  net.minecraft.world.level.storage.PrimaryLevelData worldData) {}

    static Loaded loaded;
    /** The data pack repository the data came from (a server's, for mods' set-up). */
    static PackRepository repository;

    /**
     * The open world's level stems when it was made with a mod's world preset (One Block's,
     * Grid World's: Horizon's world creation writes its id as {@code horizon:preset}), by
     * dimension; empty for the default preset, whose stems are the registries' (made at start,
     * before a world is chosen).
     */
    private static volatile Map<net.minecraft.resources.Identifier, net.minecraft.world.level.dimension.LevelStem> presetStems = Map.of();
    private static volatile String preset = "";

    /** The world's mod preset (an id, or empty for the default): its stems from now on. */
    static synchronized void usePreset(String id) {
        if (id == null) id = "";
        if (id.equals(preset)) return;
        Map<net.minecraft.resources.Identifier, net.minecraft.world.level.dimension.LevelStem> stems = new java.util.HashMap<>();
        if (!id.isEmpty() && loaded != null) {
            try {
                var key = net.minecraft.resources.ResourceKey.create(Registries.WORLD_PRESET,
                        net.minecraft.resources.Identifier.parse(id));
                var found = loaded.registries().compositeAccess().lookupOrThrow(Registries.WORLD_PRESET).get(key);
                if (found.isPresent()) {
                    found.get().value().createWorldDimensions().dimensions()
                            .forEach((k, stem) -> stems.put(k.identifier(), stem));
                    Natives.log("Java mods: the world's preset is " + id + " (" + stems.size() + " dimensions)");
                } else {
                    Natives.log("Java mods: the world's preset " + id + " isn't in the data packs; the default's dimensions");
                }
            } catch (RuntimeException e) {
                Natives.log("Java mods: the world's preset " + id + " failed: " + e);
            }
        }
        presetStems = Map.copyOf(stems);
        preset = id;
    }

    /** The world's preset (empty: the default). */
    static String preset() {
        return preset;
    }

    /**
     * Dimension {@code id}'s level stem: the world's preset's, else the registries' (the
     * default preset's); null if neither has it.
     */
    public static net.minecraft.world.level.dimension.LevelStem stem(net.minecraft.resources.Identifier id) {
        var own = presetStems.get(id);
        if (own != null) return own;
        Loaded data = loaded;
        if (data == null) return null;
        return data.registries().compositeAccess().lookupOrThrow(Registries.LEVEL_STEM)
                .get(net.minecraft.resources.ResourceKey.create(Registries.LEVEL_STEM, id))
                .map(net.minecraft.core.Holder::value).orElse(null);
    }

    /**
     * A new world's dimensions as a server with a default server.properties makes them:
     * DedicatedServerProperties from no properties (the mods' patches there decide the default
     * level type: a preset library's own, with its End and Nether biome sources), then its
     * WorldDimensionData.create. Vanilla's normal preset if that fails.
     */
    private static net.minecraft.world.level.levelgen.WorldDimensions defaultDimensions(
            net.minecraft.core.HolderLookup.Provider registries) {
        try {
            var properties = new net.minecraft.server.dedicated.DedicatedServerProperties(new java.util.Properties());
            var field = net.minecraft.server.dedicated.DedicatedServerProperties.class.getDeclaredField("worldDimensionData");
            field.setAccessible(true);
            Object data = field.get(properties);
            var create = data.getClass().getDeclaredMethod("create", net.minecraft.core.HolderLookup.Provider.class);
            create.setAccessible(true);
            var dimensions = (net.minecraft.world.level.levelgen.WorldDimensions) create.invoke(data, registries);
            var type = data.getClass().getDeclaredMethod("levelType");
            type.setAccessible(true);
            Natives.log("Java mods: a new world's level type is " + type.invoke(data) + " (the default server.properties')");
            return dimensions;
        } catch (ReflectiveOperationException | RuntimeException e) {
            Natives.log("Java mods: the default world preset failed, vanilla's normal instead: " + e);
            return registries.lookupOrThrow(Registries.WORLD_PRESET).getOrThrow(WorldPresets.NORMAL).value()
                    .createWorldDimensions();
        }
    }

    /**
     * Loads the server's data; called by Horizon (horizon-jvm) once Fabric has started.
     *
     * @param datapacks a world data pack folder: its source is what makes Fabric API add the
     *     mods' own data packs (their recipes, tags, loot, translations) to the repository
     */
    public static void prepare(String datapacks) throws Exception {
        long started = System.nanoTime();
        // Vanilla finds its data pack through the context class loader; a thread Horizon
        // attached to the JVM has none.
        Thread.currentThread().setContextClassLoader(HorizonData.class.getClassLoader());
        java.nio.file.Path folder = java.nio.file.Paths.get(datapacks);
        java.nio.file.Files.createDirectories(folder);
        PackRepository packs = repository = ServerPacksSource.createPackRepository(
                folder, new net.minecraft.world.level.validation.DirectoryValidator(path -> true));
        packs.reload();
        // A new world's packs, as vanilla picks them (MinecraftServer.configurePackRepository):
        // vanilla, then every pack whose source adds it on its own (Fabric's mod data and its
        // default-enabled built-in packs). Mods' optional built-in packs stay off: BetterEnd's
        // "flamboyant_extensions" holds recipes for a mod that may not be there, and enabling it
        // fails the whole registry load.
        WorldDataConfiguration configuration = net.minecraft.server.MinecraftServer.configurePackRepository(
                packs, new WorldDataConfiguration(DataPackConfig.DEFAULT, FeatureFlags.DEFAULT_FLAGS), false, false);
        Natives.log("data packs: " + configuration.dataPacks().getEnabled());
        WorldLoader.PackConfig packConfig = new WorldLoader.PackConfig(packs, configuration, false, true);
        WorldLoader.InitConfig init = new WorldLoader.InitConfig(
                packConfig, Commands.CommandSelection.DEDICATED, LevelBasedPermissionSet.OWNER);
        WorldLoader.WorldDataSupplier<net.minecraft.world.level.storage.PrimaryLevelData> world = context -> {
            // The preset's dimensions under the data packs' own (`dimension/overworld.json`:
            // Simply OneBlock's void overworld), as vanilla bakes a new world's.
            var dimensions = defaultDimensions(context.datapackWorldRegistries())
                    .bake(context.datapackDimensions().lookupOrThrow(Registries.LEVEL_STEM));
            // The world's settings, as the game-test server makes them. Horizon's own (game
            // mode, difficulty, rules) are mirrored over them as it hands them over.
            net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(
                    "Horizon", net.minecraft.world.level.GameType.SURVIVAL,
                    net.minecraft.world.level.LevelSettings.DifficultySettings.DEFAULT, true, configuration);
            var data = new net.minecraft.world.level.storage.PrimaryLevelData(
                    settings, dimensions.specialWorldProperty(), dimensions.lifecycle());
            return new WorldLoader.DataLoadOutput<>(data, dimensions.dimensionsRegistryAccess());
        };
        WorldLoader.ResultFactory<net.minecraft.world.level.storage.PrimaryLevelData, Loaded> result =
                (resources, reloadable, registries, data) -> new Loaded(resources, reloadable, registries, data);
        loaded = Util.blockUntilDone(executor ->
                WorldLoader.load(init, world, result, Util.backgroundExecutor(), executor)).get();
        // What MinecraftServer's constructor does next: recipe displays (recipe books) and the
        // recipe property sets (which items a furnace's or a smithing table's slots take).
        loaded.reloadable().getRecipeManager().finalizeRecipeLoading(configuration.enabledFeatures());
        Natives.log("Server data loaded in " + (System.nanoTime() - started) / 1_000_000 + " ms: "
                + loaded.reloadable().getRecipeManager().getRecipes().size() + " recipes, "
                + loaded.reloadable().getCommands().getDispatcher().getRoot().getChildren().size() + " commands");
    }
}
