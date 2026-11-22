package dev.horizonlab.platform120.mixin;

import java.util.ArrayDeque;
import net.minecraft.util.thread.BlockableEventLoop;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Work handed to the headless client or the mirror server to do "on its thread"
 * (Minecraft.execute, MinecraftServer.execute): neither runs a loop of its own -- they are
 * called, never ticked by Mojang's code -- so the work runs now, on the call that asked. Nearly
 * every packet receiver hands its work on this way.
 *
 * <p>Work handed on while other work runs waits for it to finish, as it would on a real loop:
 * with both halves in one process, a sync that answers itself (Porting Lib's tool tiers: the
 * server sends them, the client half's receiver asks the server to send them again) would
 * otherwise recurse until the stack runs out. Such a loop is cut after {@link #MOST_ROUNDS}.
 */
@Mixin(BlockableEventLoop.class)
public abstract class BlockableEventLoopMixin {
    private static final int MOST_ROUNDS = 16;
    private static final ArrayDeque<Runnable> HORIZON$LATER = new ArrayDeque<>();
    private static boolean horizon$running;
    private static boolean horizon$cutSaid;

    @Inject(method = "execute(Ljava/lang/Runnable;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$now(Runnable task, CallbackInfo ci) {
        Object self = this;
        boolean client = self == net.minecraft.client.Minecraft.getInstance()
                && dev.horizonlab.platform120.Loader120.headless();
        if (!client && self != dev.horizonlab.platform120.Mirror120.server()) {
            return;
        }
        ci.cancel();
        if (horizon$running) {
            HORIZON$LATER.add(task);
            return;
        }
        horizon$running = true;
        try {
            task.run();
            int rounds = 0;
            while (!HORIZON$LATER.isEmpty()) {
                if (++rounds > MOST_ROUNDS) {
                    if (!horizon$cutSaid) {
                        horizon$cutSaid = true;
                        dev.horizonlab.platform120.Mirror120.log("work that kept handing on more work was cut short ("
                                + HORIZON$LATER.size() + " left)");
                    }
                    HORIZON$LATER.clear();
                    break;
                }
                int now = HORIZON$LATER.size();
                for (int i = 0; i < now; i++) HORIZON$LATER.poll().run();
            }
        } finally {
            horizon$running = false;
        }
    }

    /**
     * The mirror server's and the headless client's "thread" is whichever is calling: their
     * work runs on the call that asks (above). So code that runs work at once on its own loop's
     * thread and hands it on otherwise (Forge's NetworkEvent.Context.enqueueWork) runs it at
     * once, where what it throws is seen, rather than in a future nobody reads.
     */
    @Inject(method = "isSameThread", at = @At("HEAD"), cancellable = true)
    private void horizon$sameThread(org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Boolean> cir) {
        Object self = this;
        if (self == dev.horizonlab.platform120.Mirror120.server()
                || (self == net.minecraft.client.Minecraft.getInstance() && dev.horizonlab.platform120.Loader120.headless())) {
            cir.setReturnValue(true);
        }
    }
}
