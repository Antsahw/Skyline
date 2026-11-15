package dev.horizonlab.platform120;

import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.SheetGlyphInfo;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;

/**
 * The headless font's glyphs, for what measures through the font's own font set rather than
 * its splitter: a font a mod makes from it (Jade's JadeFont asks `getFontSet`). Each glyph's
 * advance is the default font's ({@link Screens120#advance}); nothing is drawn with them, the
 * recorder takes text before it is shaped.
 */
final class MeasureFontSet120 extends FontSet {
    private MeasureFontSet120() {
        super(null, null);
    }

    static FontSet make() {
        return Allocate.make(MeasureFontSet120.class);
    }

    @Override
    public GlyphInfo getGlyphInfo(int codePoint, boolean filterFishyGlyphs) {
        float advance = Screens120.advance(codePoint);
        return new GlyphInfo() {
            @Override
            public float getAdvance() {
                return advance;
            }

            @Override
            public BakedGlyph bake(java.util.function.Function<SheetGlyphInfo, BakedGlyph> baker) {
                return null;
            }
        };
    }
}
