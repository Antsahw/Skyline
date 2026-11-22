package dev.horizonlab.platform120;

import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.EntityHitResult;

/**
 * The mod loader under the 1.20.1 platform (docs/java-bridge-120-forge.md): what the mirror asks
 * of the loader's own API (its events), so the same mirror runs under Fabric and under Forge.
 * Everything else the platform does is Minecraft 1.20.1's and the same under both.
 */
public interface Loader120 {
    /** A player uses an entity: the loader's listeners first; PASS lets vanilla's code run. */
    InteractionResult useEntity(ServerPlayer player, ServerLevel level, InteractionHand hand, Entity target, EntityHitResult hit);

    /** A player is about to break a block Horizon breaks: false when a listener stops it. */
    boolean beforeBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    /** A break {@link #beforeBreak} stopped. */
    void canceledBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    /** A block Horizon broke (`state` is what it was). */
    void afterBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    /** The loader this game runs under, found once. */
    static Loader120 get() {
        return Holder.LOADER;
    }

    final class Holder {
        private Holder() {}

        static final Loader120 LOADER = find();

        private static Loader120 find() {
            String name = classExists("net.fabricmc.loader.api.FabricLoader")
                    ? "dev.horizonlab.platform120.fabric.FabricLoader120"
                    : "dev.horizonlab.platform120.forge.ForgeLoader120";
            try {
                return (Loader120) Class.forName(name).getDeclaredConstructor().newInstance();
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException("no 1.20.1 loader side: " + name, e);
            }
        }

        private static boolean classExists(String name) {
            try {
                Class.forName(name, false, Loader120.class.getClassLoader());
                return true;
            } catch (ClassNotFoundException e) {
                return false;
            }
        }
    }
}
