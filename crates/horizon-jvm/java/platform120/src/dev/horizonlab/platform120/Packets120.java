package dev.horizonlab.platform120;

import io.netty.buffer.Unpooled;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.fabric.api.networking.v1.PacketSender;
import net.fabricmc.fabric.impl.networking.client.ClientNetworkingImpl;
import net.fabricmc.fabric.impl.networking.server.ServerNetworkingImpl;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.PacketSendListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientboundCustomPayloadPacket;
import net.minecraft.network.protocol.game.ServerboundCustomPayloadPacket;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;

/**
 * Mods' own packets between their two halves (phase 5). In 1.20.1 a mod's server half and its
 * client half talk over custom payloads: a machine sends its screen its live numbers, a mod
 * sends its config to a player who joins, a screen's button asks the server to act. Here both
 * halves run in this one runtime -- the server half on the mirror, the client half in the
 * headless client -- so a packet one sends is handed to the other's receiver directly, as Fabric
 * would hand it on after the network: through Fabric API's own registries of receivers
 * (ServerNetworkingImpl.PLAY, ClientNetworkingImpl.PLAY), each with a copy of the bytes.
 *
 * <p>A packet to a player runs the client half's receiver with the headless client standing in
 * for that player (Screens120.clientPlayer); a packet from the client half runs the server's
 * receiver as the player it stands in for. A channel nobody receives is dropped, as Fabric
 * drops one the other side never registered; anything that isn't a mod's payload (vanilla's
 * own packets to a player) is Horizon's to send, and goes nowhere here.
 */
public final class Packets120 {
    private Packets120() {}

    private static final Set<String> FAILED = ConcurrentHashMap.newKeySet();

    /** Who the client half is now. */
    public static ServerPlayer standingIn() {
        return Screens120.standingIn();
    }

    /** Whether the client half receives `channel` (ServerPlayNetworking.canSend). */
    public static boolean clientReceives(ResourceLocation channel) {
        return ClientNetworkingImpl.PLAY.getHandler(channel) != null;
    }

    /** Whether the server half receives `channel` (ClientPlayNetworking.canSend). */
    public static boolean serverReceives(ResourceLocation channel) {
        return ServerNetworkingImpl.PLAY.getHandler(channel) != null;
    }

    /** A packet the server sends `player`: a mod's payload goes to the client half's receiver. */
    public static void toClient(ServerPlayer player, Packet<?> packet) {
        if (packet instanceof net.minecraft.network.protocol.game.ClientboundOpenBookPacket book && player != null) {
            // Vanilla's one screen opened by a packet of its own (ServerPlayer.openItemGui): the
            // client shows the book in that hand, as ClientPacketListener.handleOpenBook does.
            Screens120.clientPlayer(player);
            var stack = player.getItemInHand(book.getHand());
            if (stack.is(net.minecraft.world.item.Items.WRITTEN_BOOK)) {
                net.minecraft.client.Minecraft.getInstance().setScreen(
                        new net.minecraft.client.gui.screens.inventory.BookViewScreen(
                                new net.minecraft.client.gui.screens.inventory.BookViewScreen.WrittenBookAccess(stack)));
            }
            return;
        }
        if (!(packet instanceof ClientboundCustomPayloadPacket payload)) return;
        ResourceLocation channel = payload.getIdentifier();
        var receiver = ClientNetworkingImpl.PLAY.getHandler(channel);
        if (receiver == null || player == null) return;
        FriendlyByteBuf bytes = copy(payload.getData());
        Screens120.clientPlayer(player);
        var minecraft = net.minecraft.client.Minecraft.getInstance();
        try {
            receiver.receive(minecraft, minecraft.getConnection(), bytes, clientSender(player));
        } catch (RuntimeException | LinkageError e) {
            failed("to the client on " + channel, e);
        } finally {
            bytes.release();
        }
    }

    /**
     * A packet the client half sends (ClientPacketListener.send): a mod's payload goes to the
     * server's receiver, from the player the client stands in for.
     */
    public static void fromClient(Packet<?> packet) {
        if (!(packet instanceof ServerboundCustomPayloadPacket payload)) return;
        ServerPlayer player = Screens120.standingIn();
        ResourceLocation channel = payload.getIdentifier();
        var receiver = ServerNetworkingImpl.PLAY.getHandler(channel);
        if (receiver == null || player == null) return;
        FriendlyByteBuf bytes = copy(payload.getData());
        try {
            receiver.receive(player.server, player, player.connection, bytes, serverSender(player));
        } catch (RuntimeException | LinkageError e) {
            failed("to the server on " + channel, e);
        } finally {
            bytes.release();
        }
    }

    private static FriendlyByteBuf copy(FriendlyByteBuf data) {
        return new FriendlyByteBuf(Unpooled.copiedBuffer(data.slice(data.readerIndex(), data.readableBytes())));
    }

    private static void failed(String what, Throwable e) {
        String key = what + " " + e.getClass().getName();
        if (FAILED.add(key)) {
            Mirror120.log("a mod's packet " + what + " failed: " + e);
        }
    }

    /** The server half's sender to `player` (ServerPlayNetworking.getSender, a receiver's reply). */
    public static PacketSender serverSender(ServerPlayer player) {
        return new PacketSender() {
            @Override
            public Packet<?> createPacket(ResourceLocation channel, FriendlyByteBuf buf) {
                return net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.createS2CPacket(channel, buf);
            }

            @Override
            public void sendPacket(Packet<?> packet) {
                toClient(player, packet);
            }

            @Override
            public void sendPacket(Packet<?> packet,
                    io.netty.util.concurrent.GenericFutureListener<? extends io.netty.util.concurrent.Future<? super Void>> callback) {
                toClient(player, packet);
            }

            @Override
            public void sendPacket(Packet<?> packet, PacketSendListener callback) {
                toClient(player, packet);
            }
        };
    }

    /** The client half's sender (ClientPlayNetworking.getSender, a receiver's reply). */
    public static PacketSender clientSender(ServerPlayer player) {
        return new PacketSender() {
            @Override
            public Packet<?> createPacket(ResourceLocation channel, FriendlyByteBuf buf) {
                return net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.createC2SPacket(channel, buf);
            }

            @Override
            public void sendPacket(Packet<?> packet) {
                fromClient(packet);
            }

            @Override
            public void sendPacket(Packet<?> packet,
                    io.netty.util.concurrent.GenericFutureListener<? extends io.netty.util.concurrent.Future<? super Void>> callback) {
                fromClient(packet);
            }

            @Override
            public void sendPacket(Packet<?> packet, PacketSendListener callback) {
                fromClient(packet);
            }
        };
    }
}
