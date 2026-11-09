package dev.horizonlab.platform120.mixin;

import dev.horizonlab.platform120.Mirror120;
import net.minecraft.core.RegistryAccess;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.ServerAdvancementManager;
import net.minecraft.world.item.crafting.RecipeManager;
import net.minecraft.world.level.storage.loot.LootDataManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The mirror server's data (Mirror120.data): recipes, loot tables and advancements from vanilla's
 * and the mods' data packs, loaded as a server loads them. The only server in this runtime.
 */
@Mixin(MinecraftServer.class)
public abstract class MinecraftServerMixin {
    @Inject(method = "getRecipeManager", at = @At("HEAD"), cancellable = true)
    private void horizon$recipes(CallbackInfoReturnable<RecipeManager> cir) {
        cir.setReturnValue(Mirror120.data().getRecipeManager());
    }

    /**
     * The mirror server records nothing. Its constructor never ran, so its own profiler is
     * null, and the profiler is asked for well outside profiling: Level.getEntities opens with
     * {@code getProfiler().incrementCounter("getEntities")}, which a block placed through the
     * mirror reaches (BlockItem.place -> isUnobstructed -> getEntities).
     *
     * <p>Here rather than only on Level, because mods replace {@code ServerLevel.getProfiler}
     * with one that delegates to the server (Lithium and friends merge such an override in), so
     * a fix on Level alone never runs for a ServerLevel.
     */
    @Inject(method = "getProfiler", at = @At("HEAD"), cancellable = true)
    private void horizon$profiler(
            CallbackInfoReturnable<net.minecraft.util.profiling.ProfilerFiller> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.NoProfiler.INSTANCE);
    }

    @Inject(method = "getLootData", at = @At("HEAD"), cancellable = true)
    private void horizon$loot(CallbackInfoReturnable<LootDataManager> cir) {
        cir.setReturnValue(Mirror120.data().getLootData());
    }

    @Inject(method = "getAdvancements", at = @At("HEAD"), cancellable = true)
    private void horizon$advancements(CallbackInfoReturnable<ServerAdvancementManager> cir) {
        cir.setReturnValue(Mirror120.data().getAdvancements());
    }

    @Inject(method = "registryAccess", at = @At("HEAD"), cancellable = true)
    private void horizon$registries(CallbackInfoReturnable<RegistryAccess.Frozen> cir) {
        cir.setReturnValue(Mirror120.registries());
    }
}
