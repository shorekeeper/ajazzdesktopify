#version 450

layout(location = 0) in vec2 f_uv;
layout(location = 1) in vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D atlas;

layout(location = 0) out vec4 out_color;

void main() {
    float a = texture(atlas, f_uv).r;
    out_color = vec4(f_color.rgb, f_color.a * a);
}