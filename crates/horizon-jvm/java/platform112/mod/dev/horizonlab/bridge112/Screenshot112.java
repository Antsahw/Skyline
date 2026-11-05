package dev.horizonlab.bridge112;

import java.awt.image.BufferedImage;
import java.io.File;

import javax.imageio.ImageIO;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.ScaledResolution;
import net.minecraft.client.renderer.GlStateManager;
import net.minecraft.client.renderer.RenderHelper;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.util.NonNullList;
import net.minecraft.creativetab.CreativeTabs;
import net.minecraftforge.fml.common.registry.ForgeRegistries;

/**
 * Draws one frame in the hidden GL context and saves it: the open screen (the main menu), then
 * the item icons of one mod's items (its first ten stacks) with their names, as 1.12's own renderer draws them. Proof
 * that the hidden client draws for real; later phases read screens and icons back the same way.
 */
final class Screenshot112 {
    private Screenshot112() {}

    static String take(String path, String modId) throws Exception {
        Minecraft mc = Minecraft.getMinecraft();
        int w = mc.displayWidth, h = mc.displayHeight;
        mc.getFramebuffer().bindFramebuffer(true);
        GlStateManager.clearColor(0, 0, 0, 1);
        GlStateManager.clear(16640); // colour | depth
        ScaledResolution scaled = new ScaledResolution(mc);
        mc.entityRenderer.setupOverlayRendering();
        if (mc.currentScreen != null) mc.currentScreen.drawScreen(0, 0, 0);
        NonNullList<ItemStack> stacks = NonNullList.create();
        for (Item item : ForgeRegistries.ITEMS.getValuesCollection()) {
            if (item.getRegistryName() != null && item.getRegistryName().getNamespace().equals(modId)) item.getSubItems(CreativeTabs.SEARCH, stacks);
        }
        if (stacks.size() > 10) stacks.subList(10, stacks.size()).clear();
        int x = 8, y = scaled.getScaledHeight() - 20 * stacks.size() - 8;
        net.minecraft.client.gui.Gui.drawRect(x - 4, y - 4, x + 250, y + 20 * stacks.size(), 0xE0101018);
        GlStateManager.enableDepth();
        for (ItemStack stack : stacks) {
            RenderHelper.enableGUIStandardItemLighting();
            mc.getRenderItem().renderItemAndEffectIntoGUI(stack, x, y);
            RenderHelper.disableStandardItemLighting();
            mc.fontRenderer.drawStringWithShadow(stack.getDisplayName() + "  (" + stack.getItem().getRegistryName() + ":" + stack.getMetadata() + ")", x + 20, y + 4, 0xFFFFFF);
            y += 20;
        }
        BufferedImage image = net.minecraft.util.ScreenShotHelper.createScreenshot(w, h, mc.getFramebuffer());
        ImageIO.write(image, "png", new File(path));
        return w + "x" + h + " " + stacks.size() + " stacks -> " + path;
    }
}
