package dev.horizonlab.configmod;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** A settings screen that needs its parent (reads its title, as some do) and goes back to it on Done. */
public class ConfigScreen extends Screen {
    private final Screen parent;

    ConfigScreen(String which, Screen parent) {
        super(Component.literal("Horizon Config (" + which + ")"));
        this.parent = parent;
        // Throws with no parent, as screens that read it do.
        say("horizon_config: " + which + " screen, parent " + parent.getClass().getSimpleName()
                + " titled '" + parent.getTitle().getString() + "'");
    }

    @Override
    protected void init() {
        addRenderableWidget(Button.builder(Component.literal("Done"), b -> onClose())
                .bounds(width / 2 - 50, height / 2 - 10, 100, 20).build());
    }

    private static void say(String text) {
        var player = Minecraft.getInstance().player;
        if (player != null) player.sendSystemMessage(Component.literal(text));
        System.out.println(text);
    }

    @Override
    public void onClose() {
        say("horizon_config: back to the parent");
        Minecraft.getInstance().gui.setScreen(parent);
    }
}
