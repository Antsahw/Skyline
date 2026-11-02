package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;

import java.util.AbstractMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiFunction;

/**
 * Models baked on first use, for a pack whose baked models don't fit in Java's heap
 * (HorizonClient.modelsFit): ModelBakery.bakeModels gets this map in place of baking every
 * block state and item at once (ModelBakeryMixin), and ModelManager keeps it as its model
 * set (ModelManagerMixin). A mod reading a block's model (Xaero's maps: its top texture for
 * the map's colour) bakes that state's alone, so only the states mods look at cost heap.
 * Horizon draws with its own models: this copy is only for the mods.
 */
public final class HorizonLazyModels<K, U, V> extends AbstractMap<K, V> {
    /** Whether this run bakes models on first use (set by HorizonClient.models). */
    public static volatile boolean on;

    private final Map<K, U> unbaked;
    private final BiFunction<K, U, V> bake;
    private final Map<K, V> baked = new ConcurrentHashMap<>();
    private volatile V missing;

    public HorizonLazyModels(Map<K, U> unbaked, BiFunction<K, U, V> bake) {
        this.unbaked = unbaked;
        this.bake = bake;
    }

    /** The model of keys with none (ModelManager's dispatch: the missing block model). */
    public HorizonLazyModels<K, U, V> missing(V missing) {
        this.missing = missing;
        return this;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V get(Object key) {
        V model = baked.get(key);
        if (model != null) return model;
        U source = unbaked.get(key);
        if (source == null) return missing;
        try {
            model = bake.apply((K) key, source);
        } catch (RuntimeException e) {
            Natives.log("Horizon's client: a model baked on first use failed (" + key + "): " + e);
            model = null;
        }
        if (model == null) model = missing;
        if (model == null) return null;
        V raced = baked.putIfAbsent((K) key, model);
        if (raced != null) return raced;
        int n = baked.size();
        if (n >= 256 && Integer.bitCount(n) == 1) {
            Natives.log("Horizon's client: " + n + " of " + unbaked.size() + " models baked on first use ("
                    + key.getClass().getSimpleName() + ")");
        }
        return model;
    }

    @Override
    public V getOrDefault(Object key, V fallback) {
        V model = get(key);
        return model != null ? model : fallback;
    }

    @Override
    public boolean containsKey(Object key) {
        return unbaked.containsKey(key);
    }

    @Override
    public int size() {
        return unbaked.size();
    }

    /** The models baked so far (walking every key would bake them all). */
    @Override
    public Set<Entry<K, V>> entrySet() {
        return baked.entrySet();
    }

}
