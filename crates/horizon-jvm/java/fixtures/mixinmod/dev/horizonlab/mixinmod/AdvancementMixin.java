package dev.horizonlab.mixinmod;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.advancements.AdvancementHolder;
import net.minecraft.advancements.AdvancementRewards;
import net.minecraft.network.chat.Component;
import net.minecraft.server.PlayerAdvancements;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Advancement events as Balm's and Puzzles Lib's: one heard inside PlayerAdvancements.award
 * where it is done (before its rewards: the middle of the method), and "Acquire Hardware"
 * refused at the head (counting the refusals).
 */
@Mixin(PlayerAdvancements.class)
public abstract class AdvancementMixin {
    @Shadow private ServerPlayer player;
    private static final Map<String, Integer> HORIZON_MIXINS$REFUSED = new HashMap<>();

    @Inject(method = "award", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$refuse(AdvancementHolder holder, String criterion, CallbackInfoReturnable<Boolean> cir) {
        if (holder.id().toString().equals("minecraft:story/smelt_iron")) {
            int n = HORIZON_MIXINS$REFUSED.merge(holder.id().toString(), 1, Integer::sum);
            player.sendSystemMessage(Component.literal("[mixinmod] refused " + holder.id() + " " + n));
            cir.setReturnValue(false);
        }
    }

    @Inject(method = "award", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/advancements/AdvancementRewards;grant(Lnet/minecraft/server/level/ServerPlayer;)V"))
    private void horizonMixins$earned(AdvancementHolder holder, String criterion, CallbackInfoReturnable<Boolean> cir) {
        player.sendSystemMessage(Component.literal("[mixinmod] earned " + holder.id()));
    }
}
