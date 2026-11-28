// Stand-in for reader/build.mjs inside the instrumented test copy: the same `build` and `Z`, but every outermost call is recorded.
import * as real from "ZK_BUILD_URL";
const track = globalThis.__zkTrack;
export const build = track("build", real.build);
const Zw = {};
for (const k of Object.keys(real.Z)) Zw[k] = typeof real.Z[k] === "function" ? track("Z." + k, real.Z[k], real.Z) : real.Z[k];
globalThis.__Zwrapped = Zw;
export const Z = Zw;
export const DEFAULT_RULES = real.DEFAULT_RULES;
export const STD = real.STD;
