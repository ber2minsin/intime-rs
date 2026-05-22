import { useState, useCallback } from "react";
import { TimelineRuler } from "./timeline-ruler";

function Timeline() {
    // 100 years in milliseconds
    const HUNDRED_YEARS = 100 * 365.25 * 24 * 60 * 60 * 1000;
    
    const [state, setState] = useState({
        startMs: new Date().setHours(0, 0, 0, 0),
        durationMs: 60 * 60 * 1000, // Start with 1 hour view
    });

    // We keep a ref to pass to the ruler to avoid re-mounting
    const stateRef = { current: state };

    const handleWheel = (e: React.WheelEvent) => {
        e.preventDefault();
        
        setState((prev) => {
            if (e.ctrlKey || e.metaKey) {
                // ZOOM: Scale duration
                const zoomSpeed = 0.001;
                const scale = 1 + e.deltaY * zoomSpeed;
                const newDuration = Math.max(5000, prev.durationMs * scale); // Min 5s view
                return { ...prev, durationMs: newDuration };
            } else {
                // PAN: Scale startMs based on duration
                const panSpeed = prev.durationMs / 500;
                const newStart = prev.startMs + e.deltaX * panSpeed;
                return { ...prev, startMs: newStart };
            }
        });
    };

    return (
        <div 
            className="w-full flex flex-col overflow-hidden h-32 cursor-grab active:cursor-grabbing"
            onWheel={handleWheel}
        >
            <TimelineRuler timelineState={stateRef} className="text-[#898989]" />
        </div>
    );
}

export default Timeline;