package dev.horizonlab.platform120;

import java.util.function.Supplier;
import net.minecraft.util.profiling.ProfilerFiller;
import net.minecraft.util.profiling.metrics.MetricCategory;

/**
 * A profiler that records nothing, for a mirror level (docs/java-bridge-120.md, phase 3).
 *
 * <p>A mirror level's constructor never ran, so the supplier {@code Level.getProfiler()} reads
 * is null. That matters well outside profiling: {@code Level.getEntities} opens with
 * {@code getProfiler().incrementCounter("getEntities")}, so a block item placed through the
 * mirror (BlockItem.place -> isUnobstructed -> getEntities) threw before it could place, and
 * Horizon's own placement had to stand in -- which means a mod's own placement rules never ran.
 *
 * <p>Ours rather than vanilla's {@code InactiveProfiler.INSTANCE}: that static reads as null in
 * the child (it is remapped correctly -- the bytecode asks for the right field -- so something
 * about how the class is loaded here leaves it unset), and a null profiler is the very thing
 * being fixed. One we construct cannot be null.
 */
public final class NoProfiler implements ProfilerFiller {
    public static final NoProfiler INSTANCE = new NoProfiler();

    private NoProfiler() {}

    @Override
    public void startTick() {}

    @Override
    public void endTick() {}

    @Override
    public void push(String name) {}

    @Override
    public void push(Supplier<String> name) {}

    @Override
    public void pop() {}

    @Override
    public void popPush(String name) {}

    @Override
    public void popPush(Supplier<String> name) {}

    @Override
    public void markForCharting(MetricCategory category) {}

    @Override
    public void incrementCounter(String name, int by) {}

    @Override
    public void incrementCounter(Supplier<String> name, int by) {}
}
