package dev.horizonlab.platform;

import net.minecraft.client.Minecraft;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleEngine;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.network.protocol.game.ClientboundLevelParticlesPacket;
import net.minecraft.world.entity.Entity;

/**
 * The headless client's particle engine (Minecraft.particleEngine): Horizon's game draws the
 * particles. One a mod makes from its options (createParticle: Create's belt connector, Async
 * Particles' own calls) goes to the client's player as the packet a server would send (count 0:
 * one particle, its speed the given one). A particle object a mod made itself (add) has no
 * type to send and is dropped, as are tracking emitters; the engine's maps stay empty, so mods
 * that read or tick them (Async Particles) find a real, quiet engine instead of null.
 */
public final class HorizonParticles extends ParticleEngine {
    HorizonParticles() {
        super(null, null);
    }

    @Override
    public Particle createParticle(ParticleOptions options, double x, double y, double z, double xa, double ya, double za) {
        send(options, false, false, x, y, z, xa, ya, za);
        return null;
    }

    /** A particle at a place, to the player whose client runs now (ClientLevel.doAddParticle too). */
    public static void send(ParticleOptions options, boolean force, boolean always,
                            double x, double y, double z, double xa, double ya, double za) {
        var mc = Minecraft.getInstance();
        if (mc == null || mc.player == null) return;
        var player = HorizonServer.player(mc.player.getUUID());
        if (player == null) return;
        HorizonPackets.forward(player, new ClientboundLevelParticlesPacket(
                options, force, always, x, y, z, (float) xa, (float) ya, (float) za, 1.0F, 0));
    }

    @Override
    public void add(Particle particle) {
    }

    @Override
    public void createTrackingEmitter(Entity entity, ParticleOptions particle) {
    }

    @Override
    public void createTrackingEmitter(Entity entity, ParticleOptions particle, int lifeTime) {
    }

    @Override
    public void tick() {
    }
}
