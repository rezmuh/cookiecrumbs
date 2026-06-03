package com.cookiecrumbs.demo;

import io.sentry.Sentry;
import io.sentry.SentryAttribute;
import io.sentry.SentryAttributes;
import io.sentry.metrics.SentryMetricsParameters;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;

import java.lang.management.*;
import java.util.HashMap;
import java.util.Map;

@Component
public class JvmMetricsPublisher {
    private static final Logger logger = LoggerFactory.getLogger(JvmMetricsPublisher.class);
    private static final String SERVICE_NAME = "java-spring";
    private static final String RUNTIME = "java";

    private final Map<String, Long> previousGcCounts = new HashMap<>();
    private final Map<String, Long> previousGcTimes = new HashMap<>();

    @Scheduled(fixedRate = 60_000)
    public void publishJvmMetrics() {
        try {
            emitMemoryMetrics();
            emitThreadMetrics();
            emitClassMetrics();
            emitCpuMetrics();
            emitGcMetrics();
        } catch (Exception e) {
            logger.warn("Failed to publish JVM metrics to Sentry", e);
        }
    }

    private void emitMemoryMetrics() {
        MemoryMXBean memoryMXBean = ManagementFactory.getMemoryMXBean();
        MemoryUsage heap = memoryMXBean.getHeapMemoryUsage();
        MemoryUsage nonHeap = memoryMXBean.getNonHeapMemoryUsage();

        Sentry.metrics().gauge(
            "jvm.memory.heap.used",
            Double.valueOf(heap.getUsed()),
            "byte",
            createParams()
        );
        Sentry.metrics().gauge(
            "jvm.memory.heap.max",
            Double.valueOf(heap.getMax()),
            "byte",
            createParams()
        );
        Sentry.metrics().gauge(
            "jvm.memory.non_heap.used",
            Double.valueOf(nonHeap.getUsed()),
            "byte",
            createParams()
        );
    }

    private void emitThreadMetrics() {
        ThreadMXBean threadMXBean = ManagementFactory.getThreadMXBean();
        int live = threadMXBean.getThreadCount();
        int daemon = threadMXBean.getDaemonThreadCount();

        Sentry.metrics().gauge(
            "jvm.threads.live",
            Double.valueOf(live),
            "none",
            createParams()
        );
        Sentry.metrics().gauge(
            "jvm.threads.daemon",
            Double.valueOf(daemon),
            "none",
            createParams()
        );
    }

    private void emitClassMetrics() {
        ClassLoadingMXBean classMXBean = ManagementFactory.getClassLoadingMXBean();
        int loaded = classMXBean.getLoadedClassCount();

        Sentry.metrics().gauge(
            "jvm.classes.loaded",
            Double.valueOf(loaded),
            "none",
            createParams()
        );
    }

    private void emitCpuMetrics() {
        OperatingSystemMXBean osMXBean = ManagementFactory.getOperatingSystemMXBean();
        double processCpuLoad = -1.0;
        if (osMXBean instanceof com.sun.management.OperatingSystemMXBean) {
            processCpuLoad = ((com.sun.management.OperatingSystemMXBean) osMXBean).getProcessCpuLoad();
        }
        if (processCpuLoad >= 0) {
            Sentry.metrics().gauge(
                "jvm.cpu.process",
                Double.valueOf(processCpuLoad),
                "none",
                createParams()
            );
        }
    }

    private void emitGcMetrics() {
        for (GarbageCollectorMXBean gcBean : ManagementFactory.getGarbageCollectorMXBeans()) {
            String name = gcBean.getName();
            long currentCount = gcBean.getCollectionCount();
            long currentTime = gcBean.getCollectionTime();

            if (currentCount >= 0) {
                long previousCount = previousGcCounts.getOrDefault(name, 0L);
                long deltaCount = currentCount - previousCount;
                if (deltaCount > 0) {
                    Sentry.metrics().count(
                        "jvm.gc.collections",
                        Double.valueOf(deltaCount),
                        "none",
                        createParams(name)
                    );
                }
                previousGcCounts.put(name, currentCount);
            }

            if (currentTime >= 0) {
                long previousTime = previousGcTimes.getOrDefault(name, 0L);
                long deltaTime = currentTime - previousTime;
                if (deltaTime > 0) {
                    Sentry.metrics().count(
                        "jvm.gc.collection_time",
                        Double.valueOf(deltaTime),
                        "millisecond",
                        createParams(name)
                    );
                }
                previousGcTimes.put(name, currentTime);
            }
        }
    }

    private SentryMetricsParameters createParams() {
        return SentryMetricsParameters.create(
            SentryAttributes.of(
                SentryAttribute.stringAttribute("service", SERVICE_NAME),
                SentryAttribute.stringAttribute("runtime", RUNTIME)
            )
        );
    }

    private SentryMetricsParameters createParams(String gcName) {
        return SentryMetricsParameters.create(
            SentryAttributes.of(
                SentryAttribute.stringAttribute("service", SERVICE_NAME),
                SentryAttribute.stringAttribute("runtime", RUNTIME),
                SentryAttribute.stringAttribute("collector", gcName)
            )
        );
    }
}
