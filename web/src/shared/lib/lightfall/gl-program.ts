import vertexSource from "@/shared/lib/lightfall/fullscreen.vert?raw";

/** A linked shader program, and a lookup for its uniforms by name. */
export interface Program {
  program: WebGLProgram;
  uniform: (name: string) => WebGLUniformLocation | null;
}

/** Compiles `source`. Throws if the GPU's driver can't. */
function compile(gl: WebGL2RenderingContext, type: GLenum, source: string) {
  const shader = gl.createShader(type);
  if (!shader) {
    throw new Error("no shader");
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) !== true) {
    throw new Error(gl.getShaderInfoLog(shader) ?? "shader didn't compile");
  }
  return shader;
}

/** Links `fragmentSource` with the full-screen triangle's vertex shader. Throws if it can't. */
export function link(gl: WebGL2RenderingContext, fragmentSource: string): Program {
  const vertex = compile(gl, gl.VERTEX_SHADER, vertexSource);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, fragmentSource);
  const program = gl.createProgram();
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.bindAttribLocation(program, 0, "aPosition");
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (gl.getProgramParameter(program, gl.LINK_STATUS) !== true) {
    throw new Error(gl.getProgramInfoLog(program) ?? "program didn't link");
  }
  return { program, uniform: (name) => gl.getUniformLocation(program, name) };
}

/** Where a pass draws: a framebuffer, or `null` for the canvas, and its size. */
export interface View {
  framebuffer: WebGLFramebuffer | null;
  width: number;
  height: number;
}

/** A texture of its own size to draw into, with the framebuffer that draws into it. */
export interface Target extends View {
  texture: WebGLTexture | null;
  framebuffer: WebGLFramebuffer;
}

/** An empty {@link Target}, sized on its first {@link allocate}. */
export function createTarget(gl: WebGL2RenderingContext): Target {
  return { texture: null, framebuffer: gl.createFramebuffer(), width: 0, height: 0 };
}

/** Gives `target` a texture of `size` in `format`, unless it has one that size already. */
export function allocate(
  gl: WebGL2RenderingContext,
  target: Target,
  { width, height, format }: { width: number; height: number; format: GLenum },
) {
  if (target.texture && target.width === width && target.height === height) {
    return;
  }
  gl.deleteTexture(target.texture);
  target.texture = gl.createTexture();
  target.width = width;
  target.height = height;
  gl.bindTexture(gl.TEXTURE_2D, target.texture);
  gl.texStorage2D(gl.TEXTURE_2D, 1, format, width, height);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.bindFramebuffer(gl.FRAMEBUFFER, target.framebuffer);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, target.texture, 0);
}
