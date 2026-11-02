package dev.horizonlab.platform;

import java.util.Set;
import java.util.function.Supplier;
import net.minecraft.core.Holder;
import net.minecraft.world.level.biome.Biome;

/** A biome source's possible biomes, computed once (BiomeSourceMixin). */
public final class HorizonBiomeMemo implements Supplier<Set<Holder<Biome>>> {
    /** HORIZON_JAVA_BIOME_MEMO=0: left as the mods made it (to compare). */
    public static final boolean ON = !"0".equals(System.getenv("HORIZON_JAVA_BIOME_MEMO"));

    private final Supplier<Set<Holder<Biome>>> inner;
    private volatile Set<Holder<Biome>> value;

    public HorizonBiomeMemo(Supplier<Set<Holder<Biome>>> inner) {
        this.inner = inner;
    }

    @Override
    public Set<Holder<Biome>> get() {
        Set<Holder<Biome>> v = value;
        if (v == null) {
            synchronized (this) {
                v = value;
                if (v == null) value = v = inner.get();
            }
        }
        return v;
    }
}
