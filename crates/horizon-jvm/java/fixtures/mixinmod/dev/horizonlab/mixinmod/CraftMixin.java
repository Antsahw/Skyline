package dev.horizonlab.mixinmod;

import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.ResultSlot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Inside ResultSlot.checkTakeAchievements: every crafting result taken gives an emerald. */
@Mixin(ResultSlot.class)
public abstract class CraftMixin {
    @Shadow @Final private Player player;

    @Inject(method = "checkTakeAchievements", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/world/item/ItemStack;onCraftedBy(Lnet/minecraft/world/entity/player/Player;I)V"))
    private void horizonMixins$emerald(ItemStack stack, CallbackInfo ci) {
        player.getInventory().add(new ItemStack(Items.EMERALD));
    }
}
