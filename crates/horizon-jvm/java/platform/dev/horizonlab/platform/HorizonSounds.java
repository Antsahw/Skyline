package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Field;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.sounds.AbstractSoundInstance;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.resources.sounds.TickableSoundInstance;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.protocol.game.ClientboundSoundPacket;
import net.minecraft.network.protocol.game.ClientboundStopSoundPacket;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvent;

/**
 * Sounds mods' client code plays (SoundManager.play: Create's frogport, its machines' hum, a
 * screen's button click), for the headless client, which has no sound engine: each goes to
 * the player whose client runs now as the sound packet a server would send, and Horizon's
 * game plays it. A sound at the listener (relative: UI sounds) is played at the player.
 *
 * <p>Sounds that tick (TickableSoundInstance: moving and looping sounds) are kept and ticked
 * each client tick, as the sound engine does; when one stops (isStopped, or SoundManager.stop),
 * the player's game is told to stop it (ClientboundStopSoundPacket, by its name and source).
 * Mods ask isActive before playing a sound again: a kept sound is active until it stops, a
 * one-off sound for a second. Horizon's game plays a looping sound once (its length isn't
 * known here).
 */
public final class HorizonSounds {
    private HorizonSounds() {}

    private record Playing(SoundInstance sound, UUID player, long until) {}

    private static final Map<SoundInstance, Playing> PLAYING = new ConcurrentHashMap<>();
    private static long tick;
    private static Field volumeField;
    private static Field pitchField;

    /** SoundManager.play on the headless client: whether it went to a player's game. */
    public static boolean play(SoundInstance sound) {
        var mc = Minecraft.getInstance();
        if (mc == null || mc.player == null || sound == null) return false;
        ServerPlayer player = HorizonServer.player(mc.player.getUUID());
        if (player == null) return false;
        try {
            var id = sound.getIdentifier();
            if (id == null) return false;
            Holder<SoundEvent> event = BuiltInRegistries.SOUND_EVENT.get(id)
                    .<Holder<SoundEvent>>map(h -> h)
                    .orElseGet(() -> Holder.direct(SoundEvent.createVariableRangeEvent(id)));
            double x = sound.getX(), y = sound.getY(), z = sound.getZ();
            if (sound.isRelative()) {
                x += player.getX();
                y += player.getEyeY();
                z += player.getZ();
            }
            float[] vp = volumeAndPitch(sound);
            if (vp[0] <= 0) return false;
            HorizonPackets.forward(player, new ClientboundSoundPacket(event, sound.getSource(), x, y, z, vp[0], vp[1],
                    player.getRandom().nextLong()));
            boolean ticks = sound instanceof TickableSoundInstance;
            PLAYING.put(sound, new Playing(sound, player.getUUID(), ticks ? Long.MAX_VALUE : tick + 20));
            return true;
        } catch (Throwable t) {
            Natives.log("Horizon's client: a mod's sound couldn't be played: " + t);
            return false;
        }
    }

    /** SoundManager.isActive. */
    public static boolean active(SoundInstance sound) {
        Playing p = sound == null ? null : PLAYING.get(sound);
        return p != null && p.until > tick;
    }

    /** SoundManager.stop(sound): the player's game stops it. */
    public static void stop(SoundInstance sound) {
        Playing p = sound == null ? null : PLAYING.remove(sound);
        if (p != null) stopped(p);
    }

    /** SoundManager.stop() / stopAll: every kept sound of the player whose client runs now. */
    public static void stopAll() {
        var mc = Minecraft.getInstance();
        if (mc == null || mc.player == null) return;
        UUID id = mc.player.getUUID();
        for (Playing p : PLAYING.values()) {
            if (p.player.equals(id) && PLAYING.remove(p.sound, p)) stopped(p);
        }
    }

    /** SoundManager.stop(name, source): the player's game stops those (and they are no longer kept). */
    public static void stopNamed(net.minecraft.resources.Identifier name, net.minecraft.sounds.SoundSource source) {
        var mc = Minecraft.getInstance();
        if (mc == null || mc.player == null) return;
        UUID id = mc.player.getUUID();
        for (Playing p : PLAYING.values()) {
            if (p.player.equals(id) && (name == null || name.equals(p.sound.getIdentifier()))
                    && (source == null || source == p.sound.getSource())) {
                PLAYING.remove(p.sound, p);
            }
        }
        ServerPlayer player = HorizonServer.player(id);
        if (player != null) HorizonPackets.send(player, new ClientboundStopSoundPacket(name, source));
    }

    /** A client tick of a player's kept sounds (SoundEngine.tick: tickable sounds tick, stopped ones go). */
    static void tick(UUID player) {
        for (Playing p : PLAYING.values()) {
            if (!p.player.equals(player)) continue;
            if (p.sound instanceof TickableSoundInstance tickable) {
                try {
                    if (!tickable.isStopped()) tickable.tick();
                } catch (Throwable t) {
                    PLAYING.remove(p.sound, p);
                    continue;
                }
                if (tickable.isStopped() && PLAYING.remove(p.sound, p)) stopped(p);
            } else if (p.until <= tick) {
                PLAYING.remove(p.sound, p);
            }
        }
    }

    /** The end of a client tick. */
    static void endTick() {
        tick++;
    }

    private static void stopped(Playing p) {
        ServerPlayer player = HorizonServer.player(p.player);
        if (player == null || p.sound.getIdentifier() == null) return;
        try {
            HorizonPackets.send(player, new ClientboundStopSoundPacket(p.sound.getIdentifier(), p.sound.getSource()));
        } catch (Throwable t) {
            Natives.log("Horizon's client: a mod's sound couldn't be stopped: " + t);
        }
    }

    /**
     * The sound's volume and pitch as asked for (an unresolved instance has no Sound to
     * sample: getVolume() would fail), from AbstractSoundInstance's fields, else its getters.
     */
    private static float[] volumeAndPitch(SoundInstance sound) {
        if (sound instanceof AbstractSoundInstance) {
            try {
                if (volumeField == null) {
                    volumeField = AbstractSoundInstance.class.getDeclaredField("volume");
                    volumeField.setAccessible(true);
                    pitchField = AbstractSoundInstance.class.getDeclaredField("pitch");
                    pitchField.setAccessible(true);
                }
                return new float[] {volumeField.getFloat(sound), pitchField.getFloat(sound)};
            } catch (ReflectiveOperationException e) {
                // The getters below.
            }
        }
        try {
            return new float[] {sound.getVolume(), sound.getPitch()};
        } catch (Throwable t) {
            return new float[] {1.0F, 1.0F};
        }
    }
}
