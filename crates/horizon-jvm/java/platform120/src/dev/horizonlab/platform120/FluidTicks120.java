package dev.horizonlab.platform120;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.ticks.LevelTicks;
import net.minecraft.world.ticks.ScheduledTick;

/**
 * A mirror level's fluid ticks (phase 6e): the mods' fluids' ticks, kept by position with the
 * game time they're due, and run by Horizon's ticks ({@link Mirror120#fluidTicks}). A level
 * without chunks drops what vanilla's LevelTicks is handed (it keeps ticks per chunk), so the
 * schedule is kept here: a `LiquidBlock` placed schedules its fluid (`onPlace`), as do Horizon's
 * changes beside one ({@link Mirror120#fluidChanged}). One tick per position, the earliest.
 */
final class FluidTicks120 extends LevelTicks<Fluid> {
    /** Position (`BlockPos.asLong`) to the game time its tick is due. */
    final Map<Long, Long> due = new HashMap<>();

    FluidTicks120() {
        super(pos -> true, () -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE);
    }

    @Override
    public void schedule(ScheduledTick<Fluid> tick) {
        schedule(tick.pos().asLong(), tick.triggerTick());
    }

    void schedule(long pos, long at) {
        due.merge(pos, at, Math::min);
    }

    @Override
    public boolean hasScheduledTick(BlockPos pos, Fluid fluid) {
        return due.containsKey(pos.asLong());
    }

    @Override
    public boolean willTickThisTick(BlockPos pos, Fluid fluid) {
        return due.containsKey(pos.asLong());
    }

    @Override
    public int count() {
        return due.size();
    }
}
