package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonBiomeMemo;
import java.util.Set;
import java.util.function.Supplier;
import net.minecraft.core.Holder;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeSource;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * BiomeSource.possibleBiomes computed once again. Vanilla memoizes it; TerraBlender's
 * appendDeferredBiomesList replaces the supplier with one that builds a new set at every
 * call, and Fabric's biome modifications ask it once per (modifier, biome) as the server
 * starts (BiomeSelectors.foundInOverworld): seconds with a big pack. Whatever supplier the
 * field holds is wrapped in a memo the first time it is read (again after a mod replaces it).
 */
@Mixin(BiomeSource.class)
public abstract class BiomeSourceMixin {
    @Shadow @Final @Mutable private Supplier<Set<Holder<Biome>>> possibleBiomes;

    @Inject(method = "possibleBiomes", at = @At("HEAD"), require = 0)
    private void horizon$memoizePossibleBiomes(CallbackInfoReturnable<Set<Holder<Biome>>> cir) {
        Supplier<Set<Holder<Biome>>> supplier = this.possibleBiomes;
        if (supplier != null && !(supplier instanceof HorizonBiomeMemo) && HorizonBiomeMemo.ON) {
            this.possibleBiomes = new HorizonBiomeMemo(supplier);
        }
    }
}
