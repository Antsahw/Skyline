package dev.horizonlab.platform;

import com.mojang.datafixers.DataFixer;
import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Field;
import java.util.Map;

/**
 * Bounds what DataFixerUpper keeps. It builds a rule chain the first time it fixes data of an
 * older version and keeps it for good (`DataFixerUpper.rules`, with the rewritten types in
 * `Type.REWRITE_CACHE` and `Fold`'s maps). Mods' structure templates carry ~100 different
 * DataVersions (a 148-jar pack: 10k templates), so worldgen alone kept hundreds of MB and ran
 * the heap out. The structure manager keeps each template once fixed, so a chain is rarely
 * used twice: past a few chains, they all go, and the next old template builds its own again.
 */
final class HorizonDataFixer {
    private HorizonDataFixer() {}

    /** Chains kept before the caches are cleared. */
    private static final int MAX_RULES = 6;
    /** How often to look: every 5 s at most. */
    private static final long EVERY_NANOS = 5_000_000_000L;

    private static volatile long last = System.nanoTime();
    private static int cleared;
    private static boolean broken;

    /**
     * Each server tick and each chunk Java's worldgen makes (structures load their templates
     * there, before the server's first tick): looks every 5 s.
     */
    static void check() {
        long now = System.nanoTime();
        if (broken || now - last < EVERY_NANOS) return;
        synchronized (HorizonDataFixer.class) {
            if (now - last < EVERY_NANOS) return;
            last = now;
            clearIfBig();
        }
    }

    private static void clearIfBig() {
        try {
            Map<?, ?> rules = rules(net.minecraft.util.datafix.DataFixers.getDataFixer());
            int size = rules.size();
            if (size <= MAX_RULES) return;
            rules.clear();
            clearStatic("com.mojang.datafixers.types.Type", "REWRITE_CACHE");
            clearStatic("com.mojang.datafixers.functions.Fold", "HMAP_CACHE");
            clearStatic("com.mojang.datafixers.functions.Fold", "HMAP_APPLY_CACHE");
            if (cleared++ == 0) {
                Natives.log("Java mods: the data fixer's " + size
                    + " rule chains (mods' old structure files) let go; this is done whenever more than "
                    + MAX_RULES + " are kept");
            }
        } catch (Throwable t) {
            broken = true;
            Natives.log("Java mods: the data fixer's caches can't be bounded (" + t + "); they grow as vanilla's");
        }
    }

    private static Map<?, ?> rules(DataFixer fixer) throws ReflectiveOperationException {
        Field field = com.mojang.datafixers.DataFixerUpper.class.getDeclaredField("rules");
        field.setAccessible(true);
        return (Map<?, ?>) field.get(fixer);
    }

    private static void clearStatic(String owner, String name) throws ReflectiveOperationException {
        Field field = Class.forName(owner).getDeclaredField(name);
        field.setAccessible(true);
        ((Map<?, ?>) field.get(null)).clear();
    }
}
