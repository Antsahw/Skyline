package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.ticks.LevelTicks;
import net.minecraft.world.ticks.ScheduledTick;

/**
 * A mirrored level's scheduled ticks (getBlockTicks, getFluidTicks): the level has no chunks to
 * hold them. Ticks a mod schedules for its own blocks run here, when they are due (a cabinet's
 * check for viewers), and its own fluids' (their FlowingFluid.tick: spreading by the fluid's
 * delay, slope and drop-off, as ServerLevel.tickFluid runs it); vanilla blocks' and vanilla
 * fluids' are Horizon's, and are dropped.
 */
public final class HorizonTicks<T> extends LevelTicks<T> {
    private record Due(Object type, BlockPos pos, long at) {}

    private final List<Due> due = new ArrayList<>();
    /** This level's ticks since the mirror began: the game time the mirror doesn't keep. */
    private long now;

    public HorizonTicks() {
        super(chunk -> true);
    }

    @Override
    public synchronized void schedule(ScheduledTick<T> tick) {
        if (tick.type() instanceof Block block) {
            if (BuiltInRegistries.BLOCK.getKey(block).getNamespace().equals("minecraft")) return;
        } else if (!(tick.type() instanceof Fluid fluid) || !HorizonServer.modFluid(fluid)) {
            return;
        }
        if (hasScheduledTick(tick.pos(), tick.type())) return;
        // Mods ask for getGameTime() + delay; the delay is what counts.
        long delay = Math.max(1, tick.triggerTick() - gameTime);
        due.add(new Due(tick.type(), tick.pos().immutable(), now + delay));
    }

    /** The level's game time when the mod scheduled (the mirror's levels don't advance it). */
    private long gameTime;

    @Override
    public synchronized boolean hasScheduledTick(BlockPos pos, T type) {
        for (Due d : due) if (d.type == type && d.pos.equals(pos)) return true;
        return false;
    }

    @Override
    public boolean willTickThisTick(BlockPos pos, T type) {
        return false;
    }

    @Override
    public synchronized int count() {
        return due.size();
    }

    @Override
    public void clearArea(net.minecraft.world.level.levelgen.structure.BoundingBox box) {}

    @Override
    public void copyArea(net.minecraft.world.level.levelgen.structure.BoundingBox box, net.minecraft.core.Vec3i offset) {}

    /** One server tick: the due ticks of blocks still there run (BlockState.tick). */
    public void run(ServerLevel level) {
        List<Due> ready = new ArrayList<>();
        synchronized (this) {
            now++;
            gameTime = level.getGameTime();
            for (Iterator<Due> it = due.iterator(); it.hasNext(); ) {
                Due d = it.next();
                if (d.at <= now) {
                    ready.add(d);
                    it.remove();
                }
            }
        }
        for (Due d : ready) {
            BlockState state = level.getBlockState(d.pos);
            if (d.type instanceof Fluid) {
                // ServerLevel.tickFluid: the fluid still there ticks.
                FluidState fluid = level.getFluidState(d.pos);
                if (fluid.getType() != d.type) continue;
                try {
                    fluid.tick(level, d.pos, state);
                } catch (Throwable t) {
                    Natives.log("Java mods: " + BuiltInRegistries.FLUID.getKey(fluid.getType()) + " at " + d.pos
                            + " failed its fluid tick: " + t);
                }
                continue;
            }
            if (state.getBlock() != d.type) continue;
            try {
                state.tick(level, d.pos, level.getRandom());
            } catch (Throwable t) {
                Natives.log("Java mods: " + BuiltInRegistries.BLOCK.getKey(state.getBlock()) + " at " + d.pos
                        + " failed its scheduled tick: " + t);
            }
        }
    }
}
