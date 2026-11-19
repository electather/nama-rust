import blurSource from "@/shared/lib/lightfall/blur.frag?raw";
import compositeSource from "@/shared/lib/lightfall/composite.frag?raw";
import downsampleSource from "@/shared/lib/lightfall/downsample.frag?raw";
import {
  allocate,
  createTarget,
  link,
  type Program,
  type Target,
  type View,
} from "@/shared/lib/lightfall/gl-program";
import { streakColors } from "@/shared/lib/lightfall/streak-colors";
import streaksSource from "@/shared/lib/lightfall/streaks.frag?raw";

const bloomLevels = 4;
const tau = Math.PI * 2;
// Lightfall's settings as the artboard sets them.
const rings = Math.max(1, Math.floor(tau * 0.6 + 0.5));
const period = tau / rings;

/** How far the animation has got: `intro` and `warp` run from 0 to 1. */
export interface Frame {
  travel: number;
  intro: number;
  warp: number;
}

/** Draws Lightfall frames on `canvas`. Throws where WebGL2 or its shaders fail. */
export function createRenderer(canvas: HTMLCanvasElement) {
  const gl = canvas.getContext("webgl2", { alpha: true, antialias: false, depth: false });
  if (!gl) {
    throw new Error("no WebGL2");
  }
  const streaks = link(gl, streaksSource);
  const down = link(gl, downsampleSource);
  const blur = link(gl, blurSource);
  const composite = link(gl, compositeSource);
  const triangle = gl.createBuffer();
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, triangle);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  // Half-float targets keep the bright heads from clipping; without them, light is scaled down.
  const { format, encode } =
    gl.getExtension("EXT_color_buffer_float") === null
      ? { format: gl.RGBA8, encode: 0.25 }
      : { format: gl.RGBA16F, encode: 1 };
  const scene = createTarget(gl);
  const levels = Array.from({ length: bloomLevels }, (): [Target, Target] => [
    createTarget(gl),
    createTarget(gl),
  ]);
  const { palette, top } = streakColors(canvas);

  const pass = (view: View, program: Program) => {
    gl.bindFramebuffer(gl.FRAMEBUFFER, view.framebuffer);
    gl.viewport(0, 0, view.width, view.height);
    gl.useProgram(program.program);
    return program.uniform;
  };
  const bind = (unit: number, texture: WebGLTexture | null) => {
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, texture);
  };

  const drawStreaks = (frame: Frame) => {
    const u = pass(scene, streaks);
    const stretch = 1 + 5 * (1 - frame.intro) ** 2 + 2.5 * frame.warp;
    const clock = 9 + frame.travel;
    gl.uniform2f(u("uView"), canvas.width, canvas.height);
    gl.uniform1f(u("uZoom"), 3);
    gl.uniform1f(u("uPhase"), clock % (16 * period));
    gl.uniform1f(u("uTwinkleClock"), clock % tau);
    gl.uniform1f(u("uRings"), rings);
    gl.uniform3fv(u("uColors"), palette);
    gl.uniform1i(u("uColorCount"), 2);
    gl.uniform1i(u("uStreakCount"), 2);
    gl.uniform1i(u("uColumns"), 1);
    gl.uniform1f(u("uWidth"), 5e-4 * 0.8);
    gl.uniform1f(u("uTail"), 19 / stretch);
    gl.uniform1f(u("uTwinkle"), 0.3);
    gl.uniform1f(u("uHead"), 0.6);
    gl.uniform1f(u("uDepth"), 0.5);
    gl.uniform1f(u("uEncode"), encode);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  // One direction of a separable Gaussian blur, from `from` into `into`, which share a size.
  const blurInto = (into: Target, from: Target, [x, y]: [number, number]) => {
    const b = pass(into, blur);
    bind(0, from.texture);
    gl.uniform2f(b("uTarget"), into.width, into.height);
    gl.uniform2f(b("uStep"), x, y);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  const drawBloom = () => {
    let source = scene;
    for (const [index, [front, back]] of levels.entries()) {
      const size = 2 ** (index + 1);
      const width = Math.max(1, Math.round(canvas.clientWidth / size));
      const height = Math.max(1, Math.round(canvas.clientHeight / size));
      allocate(gl, front, { width, height, format });
      allocate(gl, back, { width, height, format });
      const d = pass(front, down);
      bind(0, source.texture);
      gl.uniform2f(d("uTarget"), width, height);
      gl.uniform2f(d("uTexel"), 1 / source.width, 1 / source.height);
      gl.uniform1f(d("uLimit"), 4 * encode);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      blurInto(back, front, [1 / width, 0]);
      blurInto(front, back, [0, 1 / height]);
      source = front;
    }
  };

  const drawComposite = (frame: Frame) => {
    const c = pass({ framebuffer: null, width: canvas.width, height: canvas.height }, composite);
    bind(0, scene.texture);
    gl.uniform1i(c("uScene"), 0);
    for (const [index, [front]] of levels.entries()) {
      bind(index + 1, front.texture);
      gl.uniform1i(c(`uBloom${index}`), index + 1);
    }
    gl.uniform2f(c("uView"), canvas.width, canvas.height);
    gl.uniform1f(c("uDecode"), 1 / encode);
    gl.uniform1f(c("uGlow"), 1);
    gl.uniform1f(c("uBloom"), 0.35);
    gl.uniform1f(c("uPresence"), Math.min(1, frame.intro * 2.2));
    gl.uniform3fv(c("uTopColor"), top);
    gl.uniform1f(c("uTopGlow"), 0.06);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  return {
    render: (frame: Frame) => {
      gl.bindVertexArray(vao);
      allocate(gl, scene, { width: canvas.width, height: canvas.height, format });
      drawStreaks(frame);
      drawBloom();
      drawComposite(frame);
    },
    dispose: () => {
      for (const target of [scene, ...levels.flat()]) {
        gl.deleteTexture(target.texture);
        gl.deleteFramebuffer(target.framebuffer);
      }
      for (const program of [streaks, down, blur, composite]) {
        gl.deleteProgram(program.program);
      }
      gl.deleteBuffer(triangle);
      gl.deleteVertexArray(vao);
      // Browsers cap live WebGL contexts, so a closed page gives its one back.
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    },
  };
}
