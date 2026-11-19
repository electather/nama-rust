#version 300 es
// Adds the bloom and the glow along the top, then tone-maps, in Lightfall's dark mode.
precision highp float;
uniform sampler2D uScene;
uniform sampler2D uBloom0;
uniform sampler2D uBloom1;
uniform sampler2D uBloom2;
uniform sampler2D uBloom3;
uniform vec2 uView;
uniform float uDecode;
uniform float uGlow;
uniform float uBloom;
uniform float uPresence;
uniform vec3 uTopColor;
uniform float uTopGlow;
out vec4 outColor;

void main() {
  vec2 uv = gl_FragCoord.xy / uView;
  vec3 light = texelFetch(uScene, ivec2(gl_FragCoord.xy), 0).rgb;
  vec3 halo = texture(uBloom0, uv).rgb * 0.42;
  halo += texture(uBloom1, uv).rgb * 0.3;
  halo += texture(uBloom2, uv).rgb * 0.18;
  halo += texture(uBloom3, uv).rgb * 0.1;
  light += halo * uBloom;
  light *= uDecode * uGlow * uPresence;

  float aspect = uView.x / uView.y;
  vec2 q = vec2((uv.x - 0.5) * aspect, 1.04 - uv.y);
  float spread = 0.34 * max(aspect, 0.75);
  float core = exp(-q.x * q.x / (spread * spread) - q.y * q.y / 0.06);
  float topHalo = exp(-q.x * q.x / (3.6 * spread * spread) - q.y * q.y / 0.18);
  light += uTopColor * uTopGlow * (core * 0.85 + topHalo * 0.3) * uPresence;

  vec3 lit = max(light, 0.0);
  vec3 mapped = sqrt(1.0 - exp(-1.3 * lit * lit / (lit + 0.025)));
  float grain = (fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715)))) - 0.5) / 255.0;
  vec3 color = clamp(mapped + grain, 0.0, 1.0);
  outColor = vec4(color, max(color.r, max(color.g, color.b)));
}
