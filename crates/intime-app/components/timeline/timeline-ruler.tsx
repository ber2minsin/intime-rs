import { useCallback, useLayoutEffect, useRef } from "react";

interface RulerProps {
    timelineState: React.RefObject<{
        startMs: number;
        durationMs: number;
    }>;
    use24Hour?: boolean;
    className?: string;
}

// 1. Time Steps (in milliseconds)
const TIME_STEPS = [
    1000, 5000, 15000, 30000, 60000, 5 * 60000, 15 * 60000, 30 * 60000,
    3600000, 2 * 3600000, 6 * 3600000, 12 * 3600000, 86400000, 2 * 86400000,
    7 * 86400000, 14 * 86400000, 2592000000, 7776000000, 15552000000,
    31536000000, 63072000000, 157680000000, 315360000000
];

function TimelineRuler({ timelineState, use24Hour = false, className = "" }: RulerProps) {
    const canvasRef = useRef<HTMLCanvasElement | null>(null);

    const draw = useCallback(() => {
        const canvas = canvasRef.current;
        if (!canvas || !timelineState.current) return;

        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const { startMs, durationMs } = timelineState.current;
        const endMs = startMs + durationMs;

        const rect = canvas.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;

        canvas.width = rect.width * dpr;
        canvas.height = rect.height * dpr;

        ctx.resetTransform();
        ctx.scale(dpr, dpr);
        ctx.clearRect(0, 0, rect.width, rect.height);

        const computedColor = window.getComputedStyle(canvas).color;
        ctx.fillStyle = computedColor;
        ctx.strokeStyle = computedColor;
        ctx.lineWidth = 1;
        ctx.font = "10px sans-serif";
        ctx.textAlign = "center";
        ctx.textBaseline = "top";

        // Logic for adaptive steps
        const MIN_TICK_SPACING_PX = 80;
        const msPerPx = durationMs / rect.width;
        const targetStepMs = MIN_TICK_SPACING_PX * msPerPx;

        let majorStepMs = TIME_STEPS.find((step) => step >= targetStepMs) || TIME_STEPS[TIME_STEPS.length - 1];
        const minorStepMs = majorStepMs % 5 === 0 ? majorStepMs / 5 : majorStepMs / 4;
        const firstTickMs = Math.floor(startMs / minorStepMs) * minorStepMs;

        const timeToX = (t: number) => ((t - startMs) / durationMs) * rect.width;
        const LINE_START_Y = 24;
        const showSubticks = (minorStepMs / msPerPx) >= 8;
        // Subticks
        if (showSubticks) {
            ctx.beginPath();
            for (let time = firstTickMs; time <= endMs; time += minorStepMs) {
                if (time % majorStepMs === 0) continue;
                const x = Math.round(timeToX(time)) + 0.5;
                if (x < 0 || x > rect.width) continue;
                ctx.moveTo(x, LINE_START_Y);
                ctx.lineTo(x, rect.height);

            }
            ctx.globalAlpha = 0.25;
            ctx.stroke();
        }

        // Major Ticks & Labels
        ctx.beginPath();
        ctx.globalAlpha = 1.0;
        for (let time = firstTickMs; time <= endMs; time += minorStepMs) {
            if (time % majorStepMs !== 0) continue;

            const x = Math.round(timeToX(time)) + 0.5;
            if (x < 0 || x > rect.width) continue;

            ctx.moveTo(x, LINE_START_Y);
            ctx.lineTo(x, rect.height);
            // NEW: Draw a horizontal line at the start of the tick to cap it off
            ctx.moveTo(x - 4, LINE_START_Y); // Little cross-bar
            ctx.lineTo(x + 4, LINE_START_Y);
            // Adaptive Label Formatting
            const date = new Date(time);
            let label = "";
            if (majorStepMs < 86400000) {
                label = date.toLocaleTimeString(undefined, {
                    hour: "numeric",
                    minute: "2-digit",
                    second: majorStepMs < 60000 ? "2-digit" : undefined,
                    hour12: !use24Hour
                });
            } else if (majorStepMs < 31536000000) {
                label = date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
            } else {
                label = date.toLocaleDateString(undefined, { year: "numeric" });
            }

            ctx.fillText(label, x, 2);
        }
        ctx.stroke();
    }, [timelineState, use24Hour]);

    useLayoutEffect(() => {
        draw();
        window.addEventListener("resize", draw);
        return () => window.removeEventListener("resize", draw);
    }, [draw, timelineState.current]);

    return (
        <canvas
            ref={canvasRef}
            className={`w-full h-full [mask-image:linear-gradient(to_bottom,black_50%,transparent_100%)] ${className}`}
        />
    );
}

export { TimelineRuler };