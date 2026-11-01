package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonSurface;
import java.util.Set;
import net.minecraft.core.Holder;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldGenerationContext;
import net.minecraft.world.level.levelgen.material.MaterialSystem;
import net.minecraft.world.level.levelgen.material.rule.MaterialRule;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The surface rules' walk of a chunk: HorizonSurface's, the same blocks for less work. */
@Mixin(MaterialSystem.class)
public abstract class MaterialSystemMixin {
    @Inject(method = "buildSurface(Lnet/minecraft/world/level/levelgen/RandomState;Lnet/minecraft/world/level/biome/BiomeManager;"
            + "Lnet/minecraft/world/level/levelgen/WorldGenerationContext;Lnet/minecraft/world/level/chunk/ChunkAccess;"
            + "Lnet/minecraft/world/level/levelgen/NoiseChunk;Lnet/minecraft/world/level/levelgen/material/rule/MaterialRule;Ljava/util/Set;)V",
            at = @At("HEAD"), cancellable = true, require = 0)
    private void horizon$surface(RandomState randomState, BiomeManager biomeManager, WorldGenerationContext generation,
                                 ChunkAccess chunk, NoiseChunk noiseChunk, MaterialRule rule, Set<Holder<Biome>> possibleBiomes,
                                 CallbackInfo ci) {
        if (HorizonSurface.build((MaterialSystem) (Object) this, randomState, biomeManager, generation, chunk, noiseChunk,
                rule, possibleBiomes)) {
            ci.cancel();
        }
    }
}
