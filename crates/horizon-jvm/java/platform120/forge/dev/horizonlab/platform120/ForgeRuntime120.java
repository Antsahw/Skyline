package dev.horizonlab.platform120;

import dev.horizonlab.bridge.remote.Host;
import dev.horizonlab.bridge.remote.Peer;
import dev.horizonlab.bridge.remote.RemoteRuntime;

/**
 * Horizon's calls in the Forge 1.20.1 JVM (docs/java-bridge-120-forge.md, phase 4): what
 * Boot120 answers in the Fabric JVM, from inside the game Forge started. `start` reports the
 * mods Forge loaded; `facts` writes the content facts (Content120); every other call is
 * Mirror120's method of that name, as Boot120 calls it.
 */
final class ForgeRuntime120 implements RemoteRuntime {
    private boolean hooked;

    @Override
    public Object[] call(Peer peer, String method, Object[] args) throws Exception {
        switch (method) {
            case "ping":
                return new Object[] {"pong"};
            case "start":
            case "report":
                return new Object[] {report()};
            case "facts":
                return new Object[] {Content120.exportFacts((String) args[0])};
            default:
                return mirror(method, args);
        }
    }

    /** The mods Forge loaded, as Report120's "mods" names them: id -> {version, name}. */
    private static String report() {
        StringBuilder out = new StringBuilder("{\"loader\":\"forge\",\"mods\":{");
        boolean first = true;
        for (var mod : net.minecraftforge.fml.ModList.get().getMods()) {
            if (!first) out.append(',');
            first = false;
            out.append(quote(mod.getModId())).append(":{\"version\":").append(quote(mod.getVersion().toString()))
                    .append(",\"name\":").append(quote(mod.getDisplayName())).append('}');
        }
        return out.append("}}").toString();
    }

    private static String quote(String s) {
        StringBuilder out = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') out.append('\\').append(c);
            else if (c < 0x20) out.append(String.format("\\u%04x", (int) c));
            else out.append(c);
        }
        return out.append('"').toString();
    }

    private Object[] mirror(String method, Object[] args) throws Exception {
        if (!hooked) {
            Mirror120.call = Host::call;
            Mirror120.event = Host::event;
            hooked = true;
        }
        for (var m : Mirror120.class.getMethods()) {
            if (m.getName().equals(method) && m.getParameterCount() == args.length
                    && java.lang.reflect.Modifier.isStatic(m.getModifiers())) {
                Object[] converted = new Object[args.length];
                Class<?>[] types = m.getParameterTypes();
                for (int i = 0; i < args.length; i++) converted[i] = convert(args[i], types[i]);
                try {
                    return (Object[]) m.invoke(null, converted);
                } catch (java.lang.reflect.InvocationTargetException e) {
                    var trace = new java.io.StringWriter();
                    e.getCause().printStackTrace(new java.io.PrintWriter(trace));
                    trace.toString().lines().limit(25).forEach(l -> Host.log("Mirror120." + method + ": " + l));
                    throw e.getCause() instanceof Exception x ? x : e;
                }
            }
        }
        throw new UnsupportedOperationException("Mirror120." + method + "/" + args.length);
    }

    /** A wire value as a parameter: the wire's ints for booleans and doubles too. */
    private static Object convert(Object v, Class<?> type) {
        if (type == boolean.class) return v instanceof Boolean b ? b : ((Number) v).intValue() != 0;
        if (type == double.class) return ((Number) v).doubleValue();
        if (type == float.class) return ((Number) v).floatValue();
        if (type == int.class) return ((Number) v).intValue();
        if (type == long.class) return ((Number) v).longValue();
        return v;
    }
}
