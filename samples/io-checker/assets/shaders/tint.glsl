// One file, both stages (LÖVE 11 and 12). GLSL 1: runs on desktop OpenGL, OpenGL ES (PortMaster, mobile) and WebGL.
extern vec4 tint;
extern number wobble;
vec4 position(mat4 transform_projection, vec4 vertex_position) {
  vertex_position.y += sin(vertex_position.x * 0.05) * wobble;
  return transform_projection * vertex_position;
}
vec4 effect(vec4 color, Image tex, vec2 texture_coords, vec2 screen_coords) {
  vec4 texel = Texel(tex, texture_coords);
  float grey = dot(texel.rgb, vec3(0.299, 0.587, 0.114));
  return vec4(mix(texel.rgb, vec3(grey) * tint.rgb, tint.a), texel.a) * color;
}
