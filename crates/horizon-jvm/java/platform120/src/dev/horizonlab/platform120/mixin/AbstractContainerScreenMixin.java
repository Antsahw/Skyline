package dev.horizonlab.platform120.mixin;

import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ClickType;
import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A container screen in the headless client, where its menu is the server's own.
 *
 * <p>A click on a slot: a client sends it to the server
 * ({@code MultiPlayerGameMode.handleInventoryMouseClick}), which runs
 * {@code AbstractContainerMenu.clicked} with the clicking player. This client has no game mode
 * and no connection, and needs none: the menu is the server's, so the same method runs here,
 * for the player whose input it is.
 *
 * <p>Closing: a client closes through its own player, which this client hasn't got; the
 * player's input says it closes instead ({@code Screens120.input}), and the server's menu
 * closes as the server closes one.
 */
@Mixin(AbstractContainerScreen.class)
public abstract class AbstractContainerScreenMixin {
    @Shadow
    @Final
    protected AbstractContainerMenu menu;

    @Inject(method = "slotClicked(Lnet/minecraft/world/inventory/Slot;IILnet/minecraft/world/inventory/ClickType;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$click(Slot slot, int slotId, int button, ClickType type, CallbackInfo ci) {
        var player = dev.horizonlab.platform120.Screens120.acting();
        if (player != null) {
            this.menu.clicked(slot != null ? slot.index : slotId, button, type, player);
        }
        ci.cancel();
    }

    @Shadow
    protected Slot hoveredSlot;

    @Shadow
    protected abstract java.util.List<net.minecraft.network.chat.Component> getTooltipFromContainerItem(
            net.minecraft.world.item.ItemStack stack);

    /**
     * The hovered slot's tooltip, as vanilla makes it, with one difference: a mod whose
     * tooltip code fails here (it reads the client's player, which this client hasn't got)
     * gets the item's name alone rather than taking the whole frame down with it.
     */
    @Inject(method = "renderTooltip(Lnet/minecraft/client/gui/GuiGraphics;II)V", at = @At("HEAD"), cancellable = true)
    private void horizon$tooltip(net.minecraft.client.gui.GuiGraphics graphics, int x, int y, CallbackInfo ci) {
        ci.cancel();
        var to = dev.horizonlab.platform120.Graphics120.current();
        if (to == null || this.hoveredSlot == null || !this.hoveredSlot.hasItem() || !this.menu.getCarried().isEmpty()) {
            return;
        }
        var stack = this.hoveredSlot.getItem();
        java.util.List<net.minecraft.network.chat.Component> lines;
        try {
            lines = this.getTooltipFromContainerItem(stack);
        } catch (RuntimeException | LinkageError e) {
            lines = java.util.List.of(stack.getHoverName());
        }
        to.tooltip(lines);
    }

    @Inject(method = "onClose()V", at = @At("HEAD"), cancellable = true)
    private void horizon$close(CallbackInfo ci) {
        dev.horizonlab.platform120.Screens120.closeAsked();
        ci.cancel();
    }
}
