package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import org.apache.logging.log4j.Level;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.core.LogEvent;
import org.apache.logging.log4j.core.LoggerContext;
import org.apache.logging.log4j.core.appender.AbstractAppender;
import org.apache.logging.log4j.core.config.Property;

/**
 * Mods' log lines (log4j, and SLF4J over it) in Horizon's log. Java's log4j starts with the
 * bridge's config (horizon-jvm's {@code log4j2.xml}: the console only), not Mojang's, whose
 * file appender wrote {@code logs/latest.log} in the working folder: a dedicated server's own
 * folder, where Horizon writes its log, so both overwrote each other's lines. Once Knot is up
 * (FabricBoot), this appender takes the console's place: each line goes to Horizon's log
 * ({@code [java] [logger/LEVEL] message}), which prints it on the console as well.
 */
public final class HorizonLog extends AbstractAppender {
    private HorizonLog() {
        super("Horizon", null, null, true, Property.EMPTY_ARRAY);
    }

    /** Puts the appender on log4j's root logger in place of the console. */
    public static void install() {
        try {
            if (!(LogManager.getContext(false) instanceof LoggerContext context)) return;
            var config = context.getConfiguration();
            var appender = new HorizonLog();
            appender.start();
            config.addAppender(appender);
            var root = config.getRootLogger();
            root.addAppender(appender, null, null);
            // The console goes: Horizon's log prints the line (twice otherwise).
            root.removeAppender("SysOut");
            context.updateLoggers();
        } catch (Throwable t) {
            Natives.log("Horizon: mods' log lines stay on the console only: " + t);
        }
    }

    @Override
    public void append(LogEvent event) {
        Level level = event.getLevel();
        if (level == null || !level.isMoreSpecificThan(Level.INFO)) return;
        String logger = event.getLoggerName();
        if (logger == null || logger.isEmpty()) logger = "?";
        int dot = logger.lastIndexOf('.');
        if (dot >= 0 && dot < logger.length() - 1) logger = logger.substring(dot + 1);
        StringBuilder line = new StringBuilder();
        line.append('[').append(logger).append('/').append(level.name()).append("] ");
        line.append(event.getMessage() == null ? "" : event.getMessage().getFormattedMessage());
        Throwable thrown = event.getThrown();
        for (int depth = 0; thrown != null && depth < 4; depth++, thrown = thrown.getCause()) {
            line.append(depth == 0 ? "\n  " : "\n  caused by ").append(thrown);
            StackTraceElement[] frames = thrown.getStackTrace();
            for (int i = 0; i < Math.min(frames.length, 12); i++) line.append("\n    at ").append(frames[i]);
            if (thrown.getCause() == thrown) break;
        }
        try {
            Natives.log(line.toString());
        } catch (Throwable t) {
            // Before the natives (or after the game's end): nowhere to write it.
        }
    }
}
