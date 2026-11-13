package dev.horizonlab.platform120.mixin;

import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Where the mouse is, for screens that ask the client's mouse rather than take it (Screens120). */
@Mixin(MouseHandler.class)
public interface MouseHandlerAccess {
    @Accessor("xpos")
    void horizon$setX(double x);

    @Accessor("ypos")
    void horizon$setY(double y);
}
