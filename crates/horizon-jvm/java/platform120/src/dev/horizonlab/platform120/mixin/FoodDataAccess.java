package dev.horizonlab.platform120.mixin;

import net.minecraft.world.food.FoodData;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A player's exhaustion as Horizon has it (Hud120: AppleSkin's underlay). */
@Mixin(FoodData.class)
public interface FoodDataAccess {
    @Accessor("exhaustionLevel")
    void horizon$setExhaustion(float exhaustion);
}
