package dev.horizonlab.platform;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.horizonlab.bridge.Natives;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;

/**
 * Mods' client commands (Fabric's ClientCommandRegistrationCallback: Flashback's
 * {@code /flashback start}). A Fabric client runs them itself, before the line would go to its
 * server. Here the player's game is Horizon's, which sends every command to the server: the
 * server's command names include the client commands' (HorizonServer.commandNames, so they are
 * in the players' command tree and come to Java), and a line whose command is a client one
 * runs on that player's client (as()), with its client command source (their own connection's
 * ClientSuggestionProvider, which Fabric makes a FabricClientCommandSource); what it says goes
 * to the player's chat (ChatComponentMixin).
 */
public final class HorizonClientCommands {
    private HorizonClientCommands() {}

    private static CommandDispatcher<FabricClientCommandSource> dispatcher;

    /** The client commands' dispatcher, registered once (as Fabric does at a client's join); null without a headless client. */
    static synchronized CommandDispatcher<FabricClientCommandSource> dispatcher() {
        if (dispatcher != null || !HorizonClient.headless() || HorizonServer.server() == null) return dispatcher;
        var made = new CommandDispatcher<FabricClientCommandSource>();
        try {
            var server = HorizonServer.server();
            var context = net.minecraft.commands.CommandBuildContext.simple(server.registryAccess(), server.getWorldData().enabledFeatures());
            ClientCommandRegistrationCallback.EVENT.invoker().register(made, context);
            net.fabricmc.fabric.impl.command.client.ClientCommandInternals.setActiveDispatcher(made);
        } catch (Throwable t) {
            Natives.log("Horizon's client: mods' client commands couldn't be registered: " + t);
            HorizonFailures.report("client commands", t);
        }
        dispatcher = made;
        if (!made.getRoot().getChildren().isEmpty()) {
            Natives.log("Horizon's client: mods' client commands: " + made.getRoot().getChildren().stream()
                    .map(n -> "/" + n.getName()).sorted().toList());
        }
        return dispatcher;
    }

    /** Whether {@code line}'s command is a client one (and not the server's). */
    static boolean owns(String line, CommandDispatcher<?> server) {
        var d = dispatcher();
        if (d == null) return false;
        String root = line.split(" ", 2)[0];
        return d.getRoot().getChild(root) != null && (server == null || server.getRoot().getChild(root) == null);
    }

    /** Runs {@code line} (no slash) on the player's client, as a Fabric client runs a client command. */
    static void run(ServerPlayer player, String line) {
        try {
            HorizonClient.as(player, () -> {
                var minecraft = HorizonClient.minecraft();
                // Fabric's feedback reads the chat narrator (off: says nothing); the allocated client had none.
                if (minecraft.getNarrator() == null) {
                    var narrator = (net.minecraft.client.GameNarrator) Natives.allocate(net.minecraft.client.GameNarrator.class);
                    HorizonClient.set(net.minecraft.client.GameNarrator.class, narrator, "minecraft", minecraft);
                    HorizonClient.set(net.minecraft.client.Minecraft.class, minecraft, "narrator", narrator);
                }
                var connection = HorizonClient.connection(player);
                var source = (FabricClientCommandSource) new net.minecraft.client.multiplayer.ClientSuggestionProvider(
                        connection, minecraft, net.minecraft.server.permissions.PermissionSet.ALL_PERMISSIONS);
                try {
                    dispatcher().execute(line, source);
                } catch (CommandSyntaxException e) {
                    source.sendError(Component.literal(e.getMessage()));
                }
            });
        } catch (Throwable t) {
            Throwable cause = t.getCause() != null ? t.getCause() : t;
            Natives.tell(player.getStringUUID(), HorizonServer.chatJson(Component.literal("An error occurred while running /" + line + ": " + cause)));
            HorizonFailures.report("client command /" + line, cause);
        }
    }
}
