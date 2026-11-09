package dev.horizonlab.platform120;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.tags.TagManager;

/**
 * The block and item tags as a 1.20.1 server's data makes them, without a server or a world: the
 * data packs a server opens (vanilla's, and every Fabric mod's own data through Fabric API's pack
 * source), read with vanilla's TagLoader, which resolves tags that name tags. Experimental feature
 * packs (bundles) stay out, as in a new world.
 */
final class Tags120 {
    private Tags120() {}

    record Loaded(Map<ResourceLocation, Collection<ResourceLocation>> blocks, Map<ResourceLocation, Collection<ResourceLocation>> items) {}

    static Loaded load() {
        try (MultiPackResourceManager resources = dataPacks()) {
            return new Loaded(
                    load(resources, BuiltInRegistries.BLOCK, TagManager.getTagDir(Registries.BLOCK)),
                    load(resources, BuiltInRegistries.ITEM, TagManager.getTagDir(Registries.ITEM)));
        }
    }

    /** Vanilla's and the mods' data packs, as a new world selects them. */
    static MultiPackResourceManager dataPacks() {
        List<RepositorySource> sources = new ArrayList<>();
        sources.add(new ServerPacksSource());
        RepositorySource mods = fabricModPacks();
        if (mods != null) sources.add(mods);
        PackRepository repository = new PackRepository(sources.toArray(new RepositorySource[0]));
        repository.reload();
        List<String> selected = new ArrayList<>();
        for (Pack pack : repository.getAvailablePacks()) {
            if (pack.getRequestedFeatures().isSubsetOf(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS)) selected.add(pack.getId());
        }
        repository.setSelected(selected);
        List<PackResources> packs = repository.openAllSelected();
        return new MultiPackResourceManager(PackType.SERVER_DATA, packs);
    }

    private static <T> Map<ResourceLocation, Collection<ResourceLocation>> load(MultiPackResourceManager resources, Registry<T> registry, String dir) {
        TagLoader<ResourceLocation> loader = new TagLoader<>(id -> registry.containsKey(id) ? Optional.of(id) : Optional.empty(), dir);
        return loader.loadAndBuild(resources);
    }

    /**
     * Fabric API's source of the mods' own data packs (its resource loader adds it to the game's
     * pack repositories); {@code null} without Fabric API. Fabric's class, reached by name: the
     * platform compiles against Minecraft alone.
     */
    private static RepositorySource fabricModPacks() {
        try {
            Class<?> creator = Class.forName("net.fabricmc.fabric.impl.resource.loader.ModResourcePackCreator");
            return (RepositorySource) creator.getConstructor(PackType.class).newInstance(PackType.SERVER_DATA);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }
}
