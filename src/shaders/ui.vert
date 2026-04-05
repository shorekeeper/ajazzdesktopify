#version 450

layout(location = 0) in vec2 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(push_constant) uniform PC { vec2 screen_size; } pc;

layout(location = 0) out vec2 f_uv;
layout(location = 1) out vec4 f_color;

void main() {
    gl_Position = vec4(
        2.0 * in_pos.x / pc.screen_size.x - 1.0,
        2.0 * in_pos.y / pc.screen_size.y - 1.0,
        0.0, 1.0
    );
    f_uv = in_uv;
    f_color = in_color;
}