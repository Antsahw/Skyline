package dev.horizonlab.joinmod;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;

public final class JoinClient implements ClientModInitializer {
    /** Players whose HUD already told its waypoints. */
    private static final java.util.Set<String> HUD_TOLD = java.util.concurrent.ConcurrentHashMap.newKeySet();

    @Override
    public void onInitializeClient() {
        ClientPlayConnectionEvents.JOIN.register((handler, sender, client) ->
                say("horizon_join: fabric JOIN " + client.player.getName().getString()
                        + " connection " + (client.getConnection() == handler)
                        // The HUD's locator bar asks this every frame (ClientWaypointManager).
                        + " waypoints " + !handler.getWaypointManager().hasWaypoints()));
        // A HUD element reading the connection's waypoints as vanilla's locator bar does, once
        // per player.
        HudElementRegistry.addLast(net.minecraft.resources.Identifier.fromNamespaceAndPath("horizon_join", "waypoints"),
                (graphics, delta) -> {
                    var minecraft = net.minecraft.client.Minecraft.getInstance();
                    if (minecraft.player == null) return;
                    String name = minecraft.player.getName().getString();
                    boolean none = !minecraft.getConnection().getWaypointManager().hasWaypoints();
                    if (HUD_TOLD.add(name)) say("horizon_join: hud waypoints " + name + " " + none);
                });
    }

    private static void say(String text) {
        var player = net.minecraft.client.Minecraft.getInstance().player;
        if (player != null) player.sendSystemMessage(net.minecraft.network.chat.Component.literal(text));
        System.out.println(text);
    }
}
