package dev.horizonlab.platform120;

/** Objects made without their constructors (sun.misc.Unsafe, in jdk.unsupported). */
public final class Allocate {
    private Allocate() {}

    private static final sun.misc.Unsafe UNSAFE;

    static {
        try {
            var field = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = (sun.misc.Unsafe) field.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static <T> T make(Class<T> type) {
        try {
            return type.cast(UNSAFE.allocateInstance(type));
        } catch (InstantiationException e) {
            throw new IllegalStateException(e);
        }
    }
}
